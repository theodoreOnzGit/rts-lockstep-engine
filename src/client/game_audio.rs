// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/Audio/GameAudio.cpp (AudioManager: init's volumes, reset, update,
//     getInfoForAudioEvent, addAudioEvent, removeAudioEvent, setAudioEventEnabled, setAudioEventVolumeOverride,
//     removeDisabledEvents, isOn, setOn, setVolume, getVolume, set3DVolumeAdjustment, setListenerPosition,
//     allocateAudioRequest, appendAudioRequest, removeAllAudioRequests, getAudioLengthMS, shouldPlayLocally,
//     allocateNewHandle, loseFocus, regainFocus)
//   GeneralsMD/Code/GameEngine/Source/Common/Audio/GameSounds.cpp (SoundManager: reset, addAudioEvent,
//     notifyOf2D/3DSampleStart/Completion, getAvailableSamples, getAvailable3DSamples, canPlayNow, violatesVoice,
//     isInterrupting)
//   GeneralsMD/Code/GameEngine/Source/Common/Audio/GameMusic.cpp (MusicManager: playTrack, stopTrack, addAudioEvent,
//     removeAudioEvent)
//   GeneralsMD/Code/GameEngine/Source/Common/Audio/AudioRequest.cpp, Include/Common/AudioRequest.h,
//     Include/Common/AudioAffect.h
//   GeneralsMD/Code/GameEngineDevice/Source/MilesAudioDevice/MilesAudioManager.cpp -- read for what the device must
//     do, and its device-independent bookkeeping translated: update, stopAudio, pauseAudio, resumeAudio,
//     playAudioEvent, stopAudioEvent, killAudioEventImmediately, allocatePlayingAudio, releaseMilesHandles,
//     releasePlayingAudio, stopAllAudioImmediately, getFirst2DSample, getFirst3DSample, adjustPlayingVolume,
//     stopAllSpeech, initFilters, initFilters3D, nextMusicTrack, prevMusicTrack, isMusicPlaying,
//     hasMusicTrackCompleted, getMusicTrackName, isCurrentlyPlaying, notifyOfAudioCompletion, findPlayingAudioFrom,
//     doesViolateLimit, isPlayingAlready, isObjectPlayingVoice, findLowestPrioritySound, isPlayingLowerPriority,
//     killLowestPrioritySoundImmediately, adjustVolumeOfPlayingAudio, removePlayingAudio, removeAllDisabledAudio,
//     processRequestList, processPlayingList, has3DSensitiveStreamsPlaying, processFadingList,
//     shouldProcessRequestThisFrame, adjustRequest, checkForSample, getEffectiveVolume, startNextLoop, playStream,
//     playSample, playSample3D, initSamplePools, processRequest
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The audio manager (GitHub #13, C6): what plays, what is refused, and how loud, with the device behind a trait.
//!
//! # As upstream does it
//!
//! `TheAudio->addAudioEvent(&event)` copies the event, gives it a handle, picks its file and play info, applies a
//! script's volume override, drops it if it is not for the local player (`Type`'s `PLAYER`, `ALLIES`, `ENEMIES`,
//! `EVERYONE`) or quieter than `MinSampleVolume`, and hands it to the music manager (music) or the sound manager. The
//! sound manager's `canPlayNow` refuses a positional sound beyond its `MaxRange` from the microphone or, for a
//! `SHROUDED` one, in the local player's shroud (not `GLOBAL` or `CRITICAL` sounds); a `VOICE` sound while its object
//! already speaks (unless `INTERRUPT`); a sound over its `Limit` of the same name playing or requested (unless
//! `INTERRUPT`, which then replaces the oldest); and a sound with no free channel and nothing of lower priority to take
//! it from. Accepted events become play requests, processed once per frame: a request with a delay of a frame or more
//! waits, then checks again. Playing a sound takes a 2-D or 3-D sample from the pools (`SampleCount2D`,
//! `SampleCount3D`), killing the lowest-priority sound of its kind if none is free; music and speech are streams.
//! Each frame the playing 3-D sounds follow their owners and are dropped when quieter than `MinSampleVolume`.
//!
//! The volume of a sound is `volume * volumeShift * <system volume of its kind>`; a positional sound's is scaled by
//! `minRange / distance` beyond its `MinRange`, and is 0 at its `MaxRange` (`getEffectiveVolume`, which Miles' own
//! 3-D roll-off matches). The microphone sits between the camera's ground point and the camera
//! (`MicrophoneDesiredHeightAboveTerrain`, `MicrophoneMaxPercentageBetweenGroundAndCamera`), and the 3-D volume rises as
//! the camera zooms in (`ZoomSoundVolumePercentageAmount`).
//!
//! # Where this differs from upstream (the device is new)
//!
//! - **Miles is replaced by [`AudioDevice`]**: a voice is started on a file with a gain, a pan and a rate, and its end
//!   is reported back ([`AudioDevice::take_finished`], upstream's end-of-sample callbacks, drained at the start of
//!   [`AudioManager::update`]). The 2-D and 3-D sample pools are kept as numbered slots, so the channel limits and the
//!   priority rules are upstream's.
//! - **Positional sound is computed here, not by a 3-D provider**: a 3-D voice's gain is
//!   [`AudioManager::get_effective_volume`] each frame, and its **pan** (new) is the sine of the source's bearing from
//!   the listener's forward direction, -1 left to +1 right. Upstream's low-pass "occlusion" of off-screen sounds and the
//!   speaker types are not reproduced.
//! - **Miles' "Mono Delay" filter** is set up by `initFilters` with a wet mix of 0, so it changes nothing audible; it is
//!   not reproduced. Delays of a frame or more are upstream's request delays.
//! - **Music loops** by being restarted when it ends (upstream sets Miles' infinite loop count); each end counts towards
//!   `hasMusicTrackCompleted`.
//! - **Two upstream defects** with undefined behaviour are not copied: `killLowestPrioritySoundImmediately` erases a 2-D
//!   sound's iterator from the 3-D list (here it is removed from the 2-D list), and `processRequestList` and
//!   `processFadingList` loop forever on a `NULL` entry (none can occur here). One defect with defined behaviour is
//!   kept: `checkForSample` compares the `Type` bits with `AT_SoundEffect` (2, the `WORLD` bit), so only an event whose
//!   type is exactly `WORLD` is re-checked when its delay ends.
//! - `addAudioEvent` returns the handle even when the sound manager refused (and freed) the event, as upstream reads it
//!   from the freed copy.

use crate::client::audio_event::{
    AudioEventHost, AudioEventRts, AudioHandle, PortionToPlay, AHSV_ERROR, AHSV_FIRST_HANDLE, AHSV_MUTED, AHSV_NOT_FOR_LOCAL, AHSV_NO_SOUND,
    AHSV_STOP_THE_MUSIC, AHSV_STOP_THE_MUSIC_FADE,
};
use crate::client::audio_info::{
    AudioPriority, AudioTables, AudioType, AC_INTERRUPT, AC_LOOP, ST_ALLIES, ST_ENEMIES, ST_EVERYONE, ST_GLOBAL, ST_PLAYER, ST_SHROUDED, ST_UI, ST_VOICE,
};
use crate::common::base_type::Coord3D;
use crate::common::random_value::RandomValues;

/// `AudioAffect_Music`.
pub const AUDIO_AFFECT_MUSIC: u32 = 0x01;
/// `AudioAffect_Sound`.
pub const AUDIO_AFFECT_SOUND: u32 = 0x02;
/// `AudioAffect_Sound3D`.
pub const AUDIO_AFFECT_SOUND_3D: u32 = 0x04;
/// `AudioAffect_Speech`.
pub const AUDIO_AFFECT_SPEECH: u32 = 0x08;
/// `AudioAffect_All`.
pub const AUDIO_AFFECT_ALL: u32 = 0x0f;
/// `AudioAffect_SystemSetting`.
pub const AUDIO_AFFECT_SYSTEM_SETTING: u32 = 0x10;

/// `MSEC_PER_LOGICFRAME_REAL`.
pub const MSEC_PER_LOGICFRAME_REAL: f32 = 1000.0 / 30.0;

/// A voice the device plays (new: the Miles sample or stream handle's playing file).
pub type VoiceId = u32;

/// How a voice starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceStart {
    /// Linear gain, 0..1.
    pub gain: f32,
    /// -1 (left) .. 0 (centre) .. 1 (right).
    pub pan: f32,
    /// The playback rate's factor (`m_pitchShift`).
    pub rate: f32,
    /// What the voice is (the kind of device channel).
    pub kind: VoiceKind,
}

/// The kind of channel a voice is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceKind {
    Sample2D,
    Sample3D,
    Stream,
}

/// The device (new; upstream's Miles calls): plays files, reports their ends.
pub trait AudioDevice {
    /// Starts `file` (`loadFileForRead` + `AIL_start_*`, or `AIL_open_stream`); `None` if it cannot be played (no file).
    fn start(&mut self, file: &[u8], start: VoiceStart) -> Option<VoiceId>;
    /// The voice's gain and pan now.
    fn set_gain_pan(&mut self, voice: VoiceId, gain: f32, pan: f32);
    /// Stops the voice for good (`AIL_stop_sample` + release); no end is reported for it.
    fn stop(&mut self, voice: VoiceId);
    /// `AIL_stop_sample`/`AIL_resume_sample` (`pauseAudio`, `resumeAudio`).
    fn set_paused(&mut self, voice: VoiceId, paused: bool);
    /// The voices that reached their end since the last call (the end-of-sample callbacks).
    fn take_finished(&mut self) -> Vec<VoiceId>;
    /// `getFileLengthMS(file)`: 0 when unknown.
    fn file_length_ms(&self, file: &[u8]) -> f32;
}

/// What the manager reads of the game besides the event's owners.
pub trait AudioHost: AudioEventHost {
    /// The local player's index (`ThePlayerList->getLocalPlayer()`).
    fn local_player_index(&self) -> Option<i32>;
    /// The player the sounds are heard as: the local player, or when it is not active the observed player
    /// (`TheControlBar->getObserverLookAtPlayer()`), which may be none.
    fn listening_player_index(&self) -> Option<i32>;
    /// Whether `ThePlayerList->getNthPlayer(index)` is a player.
    fn player_exists(&self, index: i32) -> bool;
    /// Whether the player has a default team (`getDefaultTeam()`).
    fn player_has_default_team(&self, index: i32) -> bool;
    /// `owner->getRelationship(local's default team)`: `Some(true)` allies, `Some(false)` enemies, `None` neutral.
    fn are_allies(&self, owner: i32, local: i32) -> Option<bool>;
    /// `ThePartitionManager->getShroudStatusForPlayer(local, pos) == CELLSHROUD_CLEAR`.
    fn shroud_clear(&self, player: i32, pos: &Coord3D) -> bool;
    /// `TheTerrainLogic->getGroundHeight(x, y)`.
    fn ground_height(&self, x: f32, y: f32) -> f32;
}

/// `enum RequestType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestType {
    Play,
    Pause,
    Stop,
}

/// `struct AudioRequest`: an event to play, or a handle to act on.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioRequest {
    pub request: RequestType,
    /// `m_pendingEvent` (`m_usePendingEvent`), else `m_handleToInteractOn`.
    pub pending: Result<Box<AudioEventRts>, AudioHandle>,
    /// `m_requiresCheckForSample`.
    pub requires_check_for_sample: bool,
}

/// `PlayingAudioType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayingKind {
    /// `PAT_Sample` with its 2-D sample slot (`None`: no sample).
    Sample(Option<u32>),
    /// `PAT_3DSample`.
    Sample3D(Option<u32>),
    /// `PAT_Stream` (`None`: no stream opened).
    Stream(Option<VoiceId>),
}

/// `struct PlayingAudio`.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayingAudio {
    pub event: Box<AudioEventRts>,
    pub kind: PlayingKind,
    /// `m_file` (a sample's playing file; for a stream, its voice).
    pub voice: Option<VoiceId>,
    /// `m_status == PS_Stopped`.
    pub stopped: bool,
    /// `m_requestStop`.
    pub request_stop: bool,
    /// `m_framesFaded`.
    pub frames_faded: u32,
    /// `m_cleanupAudioEventRTS`.
    pub cleanup: bool,
    /// New: music ends counted (Miles' loop count).
    pub loops_completed: i32,
}

/// The volumes and switches of `AudioManager`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Volumes {
    pub music_on: bool,
    pub sound_on: bool,
    pub sound_3d_on: bool,
    pub speech_on: bool,
    pub music: f32,
    pub sound: f32,
    pub sound_3d: f32,
    pub speech: f32,
    pub system_music: f32,
    pub system_sound: f32,
    pub system_sound_3d: f32,
    pub system_speech: f32,
    pub script_music: f32,
    pub script_sound: f32,
    pub script_sound_3d: f32,
    pub script_speech: f32,
    pub zoom: f32,
}

/// The camera, as `AudioManager::update` reads `TheTacticalView`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ListenerView {
    /// `getPosition()`: the ground point looked at (its `z` is replaced by the ground height).
    pub ground: Coord3D,
    /// `getAngle()`.
    pub angle: f32,
    /// `get3DCameraPosition()`.
    pub camera: Coord3D,
}

/// `AudioManager`, `SoundManager`, `MusicManager` and the device-independent half of `MilesAudioManager`.
#[derive(Clone, Debug)]
pub struct AudioManager {
    pub tables: AudioTables,
    pub v: Volumes,
    /// `m_adjustedVolumes` (newest first).
    pub adjusted_volumes: Vec<(Vec<u8>, f32)>,
    /// `m_audioRequests`.
    pub requests: Vec<AudioRequest>,
    pub listener_position: Coord3D,
    pub listener_orientation: Coord3D,
    pub volume_has_changed: bool,
    pub disallow_speech: bool,
    /// `theAudioHandlePool`.
    pub handle_pool: AudioHandle,
    saved_values: Option<[f32; 4]>,
    // SoundManager
    pub num_2d_samples: u32,
    pub num_3d_samples: u32,
    pub num_playing_2d: u32,
    pub num_playing_3d: u32,
    // MilesAudioManager
    /// `m_availableSamples` (slots).
    pub available_samples: Vec<u32>,
    pub available_3d_samples: Vec<u32>,
    /// `getNum2DSamples()` / `getNum3DSamples()` (the pools' sizes).
    pub device_2d_samples: u32,
    pub device_3d_samples: u32,
    pub playing_sounds: Vec<PlayingAudio>,
    pub playing_3d_sounds: Vec<PlayingAudio>,
    pub playing_streams: Vec<PlayingAudio>,
    pub fading_audio: Vec<PlayingAudio>,
    /// The audio and logic random streams (`GameAudioRandomValue`; see `audio_event` for the logic one).
    pub random: RandomValues,
}

impl AudioManager {
    /// `AudioManager()` + `init()`'s volumes (the tables already loaded) + `MilesAudioManager::initSamplePools`.
    pub fn new(tables: AudioTables) -> Self {
        let s = &tables.settings;
        let v = Volumes {
            music_on: true,
            sound_on: true,
            sound_3d_on: true,
            speech_on: true,
            system_music: s.preferred_music_volume,
            system_sound: s.preferred_sound_volume,
            system_sound_3d: s.preferred_3d_sound_volume,
            system_speech: s.preferred_speech_volume,
            script_music: 1.0,
            script_sound: 1.0,
            script_sound_3d: 1.0,
            script_speech: 1.0,
            music: s.preferred_music_volume,
            sound: s.preferred_sound_volume,
            sound_3d: s.preferred_3d_sound_volume,
            speech: s.preferred_speech_volume,
            zoom: 1.0,
        };
        let n2 = s.sample_count_2d.max(0) as u32;
        let n3 = s.sample_count_3d.max(0) as u32;
        AudioManager {
            v,
            adjusted_volumes: Vec::new(),
            requests: Vec::new(),
            listener_position: Coord3D::default(),
            listener_orientation: Coord3D { x: 0.0, y: 1.0, z: 0.0 },
            volume_has_changed: false,
            disallow_speech: false,
            handle_pool: AHSV_FIRST_HANDLE,
            saved_values: None,
            num_2d_samples: 0,
            num_3d_samples: 0,
            num_playing_2d: 0,
            num_playing_3d: 0,
            available_samples: (0..n2).collect(),
            available_3d_samples: (0..n3).collect(),
            device_2d_samples: n2,
            device_3d_samples: n3,
            playing_sounds: Vec::new(),
            playing_3d_sounds: Vec::new(),
            playing_streams: Vec::new(),
            fading_audio: Vec::new(),
            random: RandomValues::new(0),
            tables,
        }
    }

    // ---------------------------------------------------------------- AudioManager (GameAudio.cpp)

    /// `getInfoForAudioEvent(event)`.
    pub fn get_info_for_audio_event(&self, e: &mut AudioEventRts) {
        if e.get_audio_event_info().is_some() {
            return;
        }
        e.event_info = self.tables.find_audio_event_info(&e.event_name).cloned();
    }

    /// `isOn(which)`.
    pub fn is_on(&self, which: u32) -> bool {
        if which & AUDIO_AFFECT_MUSIC != 0 {
            self.v.music_on
        } else if which & AUDIO_AFFECT_SOUND != 0 {
            self.v.sound_on
        } else if which & AUDIO_AFFECT_SOUND_3D != 0 {
            self.v.sound_3d_on
        } else {
            self.v.speech_on
        }
    }

    /// `setOn(on, which)`.
    pub fn set_on(&mut self, on: bool, which: u32) {
        if which & AUDIO_AFFECT_MUSIC != 0 {
            self.v.music_on = on;
        }
        if which & AUDIO_AFFECT_SOUND != 0 {
            self.v.sound_on = on;
        }
        if which & AUDIO_AFFECT_SOUND_3D != 0 {
            self.v.sound_3d_on = on;
        }
        if which & AUDIO_AFFECT_SPEECH != 0 {
            self.v.speech_on = on;
        }
    }

    /// `setVolume(volume, which)`: the system or the script's factor, the product the volume.
    pub fn set_volume(&mut self, volume: f32, which: u32) {
        let sys = which & AUDIO_AFFECT_SYSTEM_SETTING != 0;
        let v = &mut self.v;
        if which & AUDIO_AFFECT_MUSIC != 0 {
            if sys { v.system_music = volume } else { v.script_music = volume }
            v.music = v.script_music * v.system_music;
        }
        if which & AUDIO_AFFECT_SOUND != 0 {
            if sys { v.system_sound = volume } else { v.script_sound = volume }
            v.sound = v.script_sound * v.system_sound;
        }
        if which & AUDIO_AFFECT_SOUND_3D != 0 {
            if sys { v.system_sound_3d = volume } else { v.script_sound_3d = volume }
            v.sound_3d = v.script_sound_3d * v.system_sound_3d;
        }
        if which & AUDIO_AFFECT_SPEECH != 0 {
            if sys { v.system_speech = volume } else { v.script_speech = volume }
            v.speech = v.script_speech * v.system_speech;
        }
        self.volume_has_changed = true;
    }

    /// `getVolume(which)`.
    pub fn get_volume(&self, which: u32) -> f32 {
        if which & AUDIO_AFFECT_MUSIC != 0 {
            self.v.music
        } else if which & AUDIO_AFFECT_SOUND != 0 {
            self.v.sound
        } else if which & AUDIO_AFFECT_SOUND_3D != 0 {
            self.v.sound_3d
        } else {
            self.v.speech
        }
    }

    /// `set3DVolumeAdjustment(adj)`: the 3-D volume times the zoom factor, clamped to [0, 1].
    pub fn set_3d_volume_adjustment(&mut self, adj: f32) {
        self.v.sound_3d = (adj * self.v.script_sound_3d * self.v.system_sound_3d).clamp(0.0, 1.0);
        if !self.has_3d_sensitive_streams_playing() {
            self.volume_has_changed = true;
        }
    }

    /// `reset()` (with `MilesAudioManager::reset`'s `stopAllAudioImmediately` and `removeAllAudioRequests`).
    pub fn reset<D: AudioDevice>(&mut self, device: &mut D) {
        self.adjusted_volumes.clear();
        let v = &mut self.v;
        v.script_music = 1.0;
        v.script_sound = 1.0;
        v.script_sound_3d = 1.0;
        v.script_speech = 1.0;
        v.music = v.system_music;
        v.sound = v.system_sound;
        v.sound_3d = v.system_sound_3d;
        v.speech = v.system_speech;
        self.disallow_speech = false;
        self.stop_all_audio_immediately(device);
        self.requests.clear();
    }

    /// `allocateNewHandle()`: post-increment.
    fn allocate_new_handle(&mut self) -> AudioHandle {
        let h = self.handle_pool;
        self.handle_pool = self.handle_pool.wrapping_add(1);
        h
    }

    /// `addAudioEvent(event)` (see the module docs). The caller's event gets the file index chosen
    /// (`setPlayingAudioIndex`).
    pub fn add_audio_event(&mut self, host: &dyn AudioHost, event: &mut AudioEventRts) -> AudioHandle {
        if event.event_name.is_empty() || event.event_name == b"NoSound" {
            return AHSV_NO_SOUND;
        }
        if event.get_audio_event_info().is_none() {
            self.get_info_for_audio_event(event);
            if event.event_info.is_none() {
                return AHSV_ERROR;
            }
        }
        let ty = event.info().sound_type;
        match ty {
            AudioType::Music if !self.is_on(AUDIO_AFFECT_MUSIC) => return AHSV_NO_SOUND,
            AudioType::SoundEffect if !self.is_on(AUDIO_AFFECT_SOUND) || !self.is_on(AUDIO_AFFECT_SOUND_3D) => return AHSV_NO_SOUND,
            AudioType::Streaming if !self.is_on(AUDIO_AFFECT_SPEECH) => return AHSV_NO_SOUND,
            _ => {}
        }
        if self.disallow_speech && ty == AudioType::Streaming {
            return AHSV_NO_SOUND;
        }
        let mut e = Box::new(event.clone());
        e.playing_handle = self.allocate_new_handle();
        e.generate_filename(&self.tables.settings, host, &mut self.random);
        event.playing_audio_index = e.playing_audio_index;
        e.generate_play_info(&self.tables.settings, host, &mut self.random);
        if let Some((_, v)) = self.adjusted_volumes.iter().find(|(n, _)| *n == e.event_name) {
            e.volume = *v;
        }
        if !e.uninterruptable && !self.should_play_locally(host, &e) {
            return AHSV_NOT_FOR_LOCAL;
        }
        if e.get_volume() < self.tables.settings.min_volume {
            return AHSV_MUTED;
        }
        let handle = e.playing_handle;
        if ty == AudioType::Music {
            // MusicManager::addAudioEvent -> playTrack
            self.requests.push(AudioRequest { request: RequestType::Play, pending: Ok(e), requires_check_for_sample: false });
        } else {
            self.sound_add_audio_event(host, e);
        }
        handle
    }

    /// `removeAudioEvent(handle)`: the music's special handles go to the music manager at once; any other handle gets a
    /// stop request.
    pub fn remove_audio_event<D: AudioDevice>(&mut self, device: &mut D, handle: AudioHandle) {
        if handle == AHSV_STOP_THE_MUSIC || handle == AHSV_STOP_THE_MUSIC_FADE {
            // MusicManager::removeAudioEvent -> stopTrack: a stop request
            self.requests.push(AudioRequest { request: RequestType::Stop, pending: Err(handle), requires_check_for_sample: false });
            let _ = device;
            return;
        }
        if handle < AHSV_FIRST_HANDLE {
            return;
        }
        self.requests.push(AudioRequest { request: RequestType::Stop, pending: Err(handle), requires_check_for_sample: false });
    }

    /// `removeAudioEvent(name)` -> `removePlayingAudio(name)`.
    pub fn remove_audio_event_named<D: AudioDevice>(&mut self, device: &mut D, name: &[u8]) {
        self.remove_playing_audio(device, name);
    }

    /// `setAudioEventEnabled(name, enable)`.
    pub fn set_audio_event_enabled(&mut self, name: &[u8], enable: bool) {
        self.set_audio_event_volume_override(name, if enable { -1.0 } else { 0.0 });
    }

    /// `setAudioEventVolumeOverride(name, volume)`: -1 removes the override; an empty name clears them all.
    pub fn set_audio_event_volume_override(&mut self, name: &[u8], volume: f32) {
        if name.is_empty() {
            self.adjusted_volumes.clear();
            return;
        }
        if volume != -1.0 {
            self.adjust_volume_of_playing_audio(name, volume);
        }
        if let Some(i) = self.adjusted_volumes.iter().position(|(n, _)| n == name) {
            if volume == -1.0 {
                self.adjusted_volumes.remove(i);
            } else {
                self.adjusted_volumes[i].1 = volume;
            }
            return;
        }
        if volume != -1.0 {
            self.adjusted_volumes.insert(0, (name.to_vec(), volume));
        }
    }

    /// `removeDisabledEvents()` -> `removeAllDisabledAudio()`.
    pub fn remove_disabled_events<D: AudioDevice>(&mut self, device: &mut D) {
        self.remove_all_disabled_audio(device);
    }

    /// `shouldPlayLocally(event)`.
    pub fn should_play_locally(&self, host: &dyn AudioHost, e: &AudioEventRts) -> bool {
        let local = host.listening_player_index();
        let ei = e.info();
        if ei.sound_type == AudioType::Music {
            return true;
        }
        let t = ei.sound_type_bits;
        if t & (ST_PLAYER | ST_ALLIES | ST_ENEMIES | ST_EVERYONE) == 0 {
            return true;
        }
        if t & ST_EVERYONE != 0 {
            return true;
        }
        let pi = e.get_player_index(host);
        let owner = host.player_exists(pi).then_some(pi);
        if t & ST_PLAYER != 0 && t & ST_UI != 0 && owner.is_none() {
            return true;
        }
        let Some(owner) = owner else { return false };
        let Some(local) = local else { return false };
        if !host.player_has_default_team(local) {
            return false;
        }
        if t & ST_PLAYER != 0 {
            return owner == local;
        }
        if t & ST_ALLIES != 0 {
            return owner != local && host.are_allies(owner, local) == Some(true);
        }
        if t & ST_ENEMIES != 0 {
            return host.are_allies(owner, local) == Some(false);
        }
        false
    }

    /// `update()` + `MilesAudioManager::update()`: the device's ended voices, the microphone and zoom, then the request,
    /// playing and fading lists.
    pub fn update<D: AudioDevice>(&mut self, host: &dyn AudioHost, device: &mut D, view: &ListenerView) {
        for voice in device.take_finished() {
            self.notify_of_audio_completion(host, device, voice);
        }
        self.update_listener(host, view);
        self.process_request_list(host, device);
        self.process_playing_list(host, device);
        self.process_fading_list(device);
    }

    /// `AudioManager::update`'s microphone and zoom volume.
    pub fn update_listener(&mut self, host: &dyn AudioHost, view: &ListenerView) {
        let mut ground = view.ground;
        let angle = view.angle;
        // Matrix3D::Rotate_Z(angle) applied to (0, 1, 0)
        let forward = Coord3D { x: -crate::ww::wwmath::sin(angle), y: crate::ww::wwmath::cos(angle), z: 0.0 };
        let s = &self.tables.settings;
        let desired_height = s.microphone_desired_height_above_terrain;
        let max_percentage = s.microphone_max_percentage_between_ground_and_camera;
        let camera = view.camera;
        let mut g2c = Coord3D { x: camera.x - ground.x, y: camera.y - ground.y, z: camera.z - ground.z };
        let best = if camera.z <= desired_height || g2c.z <= 0.0 { max_percentage } else { max_percentage.min(desired_height / g2c.z) };
        g2c.x *= best;
        g2c.y *= best;
        g2c.z *= best;
        ground.z = host.ground_height(ground.x, ground.y);
        let mic = Coord3D { x: ground.x + g2c.x, y: ground.y + g2c.y, z: ground.z + g2c.z };
        self.listener_position = mic;
        self.listener_orientation = forward;
        let max_boost = s.zoom_sound_volume_percentage_amount;
        let (min_dist, max_dist) = (s.zoom_min_distance, s.zoom_max_distance);
        self.v.zoom = 1.0 - max_boost;
        if max_boost > 0.0 {
            let dist = Coord3D { x: camera.x - mic.x, y: camera.y - mic.y, z: camera.z - mic.z }.length();
            if dist < min_dist {
                self.v.zoom = 1.0;
            } else if dist < max_dist {
                let scalar = (dist - min_dist) / (max_dist - min_dist);
                self.v.zoom = 1.0 - scalar * max_boost;
            }
        }
        self.set_3d_volume_adjustment(self.v.zoom);
    }

    /// `getAudioLengthMS(event)`: the attack, sound and decay files' lengths, from a copy (its draws advance the
    /// generators, as upstream's do).
    pub fn get_audio_length_ms<D: AudioDevice>(&mut self, host: &dyn AudioHost, device: &D, event: &AudioEventRts) -> f32 {
        let mut e = event.clone();
        if e.get_audio_event_info().is_none() {
            self.get_info_for_audio_event(&mut e);
            if e.event_info.is_none() {
                return 0.0;
            }
        }
        e.generate_filename(&self.tables.settings, host, &mut self.random);
        e.generate_play_info(&self.tables.settings, host, &mut self.random);
        let len = |f: &[u8]| if f.is_empty() { 0.0 } else { device.file_length_ms(f) };
        len(&e.attack_name) + len(&e.filename_to_load) + len(&e.decay_name)
    }

    /// `loseFocus()`: every system volume to 0, the old ones kept.
    pub fn lose_focus(&mut self) {
        self.saved_values = Some([self.v.system_music, self.v.system_sound, self.v.system_sound_3d, self.v.system_speech]);
        self.set_volume(0.0, AUDIO_AFFECT_ALL | AUDIO_AFFECT_SYSTEM_SETTING);
    }

    /// `regainFocus()`.
    pub fn regain_focus(&mut self) {
        let Some(s) = self.saved_values.take() else { return };
        self.set_volume(s[0], AUDIO_AFFECT_MUSIC | AUDIO_AFFECT_SYSTEM_SETTING);
        self.set_volume(s[1], AUDIO_AFFECT_SOUND | AUDIO_AFFECT_SYSTEM_SETTING);
        self.set_volume(s[2], AUDIO_AFFECT_SOUND_3D | AUDIO_AFFECT_SYSTEM_SETTING);
        self.set_volume(s[3], AUDIO_AFFECT_SPEECH | AUDIO_AFFECT_SYSTEM_SETTING);
    }

    // ---------------------------------------------------------------- SoundManager (GameSounds.cpp)

    /// `SoundManager::addAudioEvent`: a play request if `canPlayNow`, else the event is dropped.
    fn sound_add_audio_event(&mut self, host: &dyn AudioHost, mut e: Box<AudioEventRts>) {
        if self.num_2d_samples == 0 && self.num_3d_samples == 0 {
            self.num_2d_samples = self.device_2d_samples;
            self.num_3d_samples = self.device_3d_samples;
        }
        if self.can_play_now(host, &mut e) {
            self.requests.push(AudioRequest { request: RequestType::Play, pending: Ok(e), requires_check_for_sample: false });
        }
    }

    /// `SoundManager::canPlayNow(event)` (see the module docs).
    pub fn can_play_now(&self, host: &dyn AudioHost, e: &mut AudioEventRts) -> bool {
        let info = e.event_info.clone().expect("described");
        if e.is_positional_audio() && info.sound_type_bits & ST_GLOBAL == 0 && info.priority != AudioPriority::Critical {
            if let Some(pos) = e.get_current_position(host) {
                let d = Coord3D { x: self.listener_position.x - pos.x, y: self.listener_position.y - pos.y, z: self.listener_position.z - pos.z };
                if d.length() >= info.max_distance {
                    return false;
                }
                let local = host.local_player_index().unwrap_or(0);
                if info.sound_type_bits & ST_SHROUDED != 0 && !host.shroud_clear(local, &pos) {
                    return false;
                }
            }
        }
        if self.violates_voice(e) {
            return Self::is_interrupting(e);
        }
        if self.does_violate_limit(e) {
            return false;
        } else if Self::is_interrupting(e) {
            return true;
        }
        if e.is_positional_audio() {
            if self.num_playing_3d < self.num_3d_samples {
                return true;
            }
        } else if self.num_playing_2d < self.num_2d_samples {
            return true;
        }
        if self.is_playing_lower_priority(e) {
            return true;
        }
        if Self::is_interrupting(e) {
            return self.is_playing_already(e);
        }
        false
    }

    /// `SoundManager::violatesVoice`.
    fn violates_voice(&self, e: &AudioEventRts) -> bool {
        if e.info().sound_type_bits & ST_VOICE != 0 {
            return e.get_object_id() != 0 && self.is_object_playing_voice(e.get_object_id());
        }
        false
    }

    /// `SoundManager::isInterrupting`.
    fn is_interrupting(e: &AudioEventRts) -> bool {
        e.info().control & AC_INTERRUPT != 0
    }

    /// `notifyOf2DSampleCompletion` / `notifyOf3DSampleCompletion`.
    fn notify_sample_completion(&mut self, three_d: bool) {
        let n = if three_d { &mut self.num_playing_3d } else { &mut self.num_playing_2d };
        if *n > 0 {
            *n -= 1;
        }
    }

    // ---------------------------------------------------------------- MilesAudioManager's bookkeeping

    /// The list a 2-D or 3-D event is on.
    fn list_for(&self, e: &AudioEventRts) -> &Vec<PlayingAudio> {
        if e.is_positional_audio() {
            &self.playing_3d_sounds
        } else {
            &self.playing_sounds
        }
    }

    /// `doesViolateLimit(event)`: sets the event's handle to kill to the oldest of its name playing.
    pub fn does_violate_limit(&self, e: &mut AudioEventRts) -> bool {
        let limit = e.info().limit;
        if limit == 0 {
            return false;
        }
        let mut total = 0;
        let mut total_requests = 0;
        for p in self.list_for(e) {
            if p.event.event_name == e.event_name {
                if total == 0 {
                    e.kill_this_handle = p.event.playing_handle;
                }
                total += 1;
            }
        }
        for r in &self.requests {
            if let Ok(pe) = &r.pending {
                if pe.event_name == e.event_name {
                    total_requests += 1;
                    total += 1;
                }
            }
        }
        if e.info().control & AC_INTERRUPT != 0 && total_requests < limit {
            let playing = total - total_requests;
            if total_requests + playing < limit {
                e.kill_this_handle = 0;
                return false;
            }
            return false;
        }
        if total < limit {
            e.kill_this_handle = 0;
            return false;
        }
        true
    }

    /// `isPlayingAlready(event)`.
    pub fn is_playing_already(&self, e: &AudioEventRts) -> bool {
        self.list_for(e).iter().any(|p| p.event.event_name == e.event_name)
    }

    /// `isObjectPlayingVoice(objID)`.
    pub fn is_object_playing_voice(&self, obj: u32) -> bool {
        if obj == 0 {
            return false;
        }
        self.playing_sounds.iter().chain(&self.playing_3d_sounds).any(|p| p.event.get_object_id() == obj && p.event.info().sound_type_bits & ST_VOICE != 0)
    }

    /// `findLowestPrioritySound(event)`: the index in its list of the first sound of the lowest priority below the event's.
    fn find_lowest_priority_sound(&self, e: &AudioEventRts) -> Option<usize> {
        let priority = e.info().priority;
        if priority == AudioPriority::Lowest {
            return None;
        }
        let mut lowest: Option<(usize, AudioPriority)> = None;
        for (i, p) in self.list_for(e).iter().enumerate() {
            let pp = p.event.info().priority;
            if pp < priority && lowest.is_none_or(|(_, l)| l > pp) {
                lowest = Some((i, pp));
                if pp == AudioPriority::Lowest {
                    return Some(i);
                }
            }
        }
        lowest.map(|(i, _)| i)
    }

    /// `isPlayingLowerPriority(event)`.
    pub fn is_playing_lower_priority(&self, e: &AudioEventRts) -> bool {
        let priority = e.info().priority;
        if priority == AudioPriority::Lowest {
            return false;
        }
        self.list_for(e).iter().any(|p| p.event.info().priority < priority)
    }

    /// `killLowestPrioritySoundImmediately(event)` (see the module docs for the 2-D list).
    fn kill_lowest_priority_sound_immediately<D: AudioDevice>(&mut self, device: &mut D, e: &AudioEventRts) -> bool {
        let Some(i) = self.find_lowest_priority_sound(e) else { return false };
        let p = if e.is_positional_audio() { self.playing_3d_sounds.remove(i) } else { self.playing_sounds.remove(i) };
        self.release_playing_audio(device, p);
        true
    }

    /// `releasePlayingAudio(audio)` (+ `releaseMilesHandles`, `closeFile`): the sample counts, the slot back to its
    /// pool, the voice stopped.
    fn release_playing_audio<D: AudioDevice>(&mut self, device: &mut D, p: PlayingAudio) {
        if p.event.info().sound_type == AudioType::SoundEffect {
            match p.kind {
                PlayingKind::Sample(Some(_)) => self.notify_sample_completion(false),
                PlayingKind::Sample3D(Some(_)) => self.notify_sample_completion(true),
                _ => {}
            }
        }
        match p.kind {
            PlayingKind::Sample(Some(s)) => self.available_samples.push(s),
            PlayingKind::Sample3D(Some(s)) => self.available_3d_samples.push(s),
            _ => {}
        }
        if let Some(v) = p.voice {
            device.stop(v);
        }
    }

    /// `stopAllAudioImmediately()`.
    pub fn stop_all_audio_immediately<D: AudioDevice>(&mut self, device: &mut D) {
        for list in [0, 1, 2, 3] {
            let l = match list {
                0 => std::mem::take(&mut self.playing_sounds),
                1 => std::mem::take(&mut self.playing_3d_sounds),
                2 => std::mem::take(&mut self.playing_streams),
                _ => std::mem::take(&mut self.fading_audio),
            };
            for p in l {
                self.release_playing_audio(device, p);
            }
        }
    }

    /// `adjustPlayingVolume(audio)`.
    fn adjust_playing_volume<D: AudioDevice>(&self, device: &mut D, p: &PlayingAudio, pan: f32) {
        let Some(v) = p.voice else { return };
        let desired = p.event.get_volume() * p.event.volume_shift;
        let gain = match p.kind {
            PlayingKind::Sample(_) => self.v.sound * desired,
            PlayingKind::Sample3D(_) => self.v.sound_3d * desired,
            PlayingKind::Stream(_) => {
                if p.event.info().sound_type == AudioType::Music {
                    self.v.music * desired
                } else {
                    self.v.speech * desired
                }
            }
        };
        device.set_gain_pan(v, gain, pan);
    }

    /// `stopAllSpeech()`.
    fn stop_all_speech<D: AudioDevice>(&mut self, device: &mut D) {
        let (speech, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.playing_streams).into_iter().partition(|p| p.event.info().sound_type == AudioType::Streaming);
        self.playing_streams = keep;
        for p in speech {
            self.release_playing_audio(device, p);
        }
    }

    /// The pan of a position (new, see the module docs): the sine of its bearing from the listener's forward direction.
    pub fn pan_of(&self, pos: &Coord3D) -> f32 {
        let dx = pos.x - self.listener_position.x;
        let dy = pos.y - self.listener_position.y;
        let d = Coord3D { x: dx, y: dy, z: 0.0 }.length();
        if d <= 0.0 {
            return 0.0;
        }
        let f = &self.listener_orientation;
        // the listener's right: forward turned a quarter clockwise about +z
        let (rx, ry) = (f.y, -f.x);
        ((dx * rx + dy * ry) / d).clamp(-1.0, 1.0)
    }

    /// `playSample(event, sample)`: the 2-D voice (`initFilters`: volume `volume * volumeShift * soundVolume`, centred,
    /// rate by the pitch shift).
    fn play_sample<D: AudioDevice>(&self, device: &mut D, e: &AudioEventRts) -> Option<VoiceId> {
        let gain = e.get_volume() * e.volume_shift * self.v.sound;
        device.start(Self::portion_file(e), VoiceStart { gain, pan: 0.0, rate: e.pitch_shift, kind: VoiceKind::Sample2D })
    }

    /// `playSample3D(event, sample)`: none without a position; else the 3-D voice at its effective volume and pan.
    fn play_sample_3d<D: AudioDevice>(&self, host: &dyn AudioHost, device: &mut D, e: &mut AudioEventRts) -> Option<VoiceId> {
        let pos = if e.is_positional_audio() { e.get_current_position(host) } else { None };
        let pos = pos?;
        let gain = self.get_effective_volume(host, e);
        device.start(Self::portion_file(e), VoiceStart { gain, pan: self.pan_of(&pos), rate: e.pitch_shift, kind: VoiceKind::Sample3D })
    }

    /// The file of the portion to play next (Miles' `AudioFileCache::openFile`: the attack, the sound or the decay).
    fn portion_file(e: &AudioEventRts) -> &[u8] {
        match e.portion_to_play_next {
            PortionToPlay::Attack => &e.attack_name,
            PortionToPlay::Decay => &e.decay_name,
            _ => &e.filename_to_load,
        }
    }

    /// `playAudioEvent(event)`.
    fn play_audio_event<D: AudioDevice>(&mut self, host: &dyn AudioHost, device: &mut D, mut e: Box<AudioEventRts>) {
        let info = e.event_info.clone().expect("described");
        let handle_to_kill = e.kill_this_handle;
        match info.sound_type {
            AudioType::Music | AudioType::Streaming => {
                if info.sound_type == AudioType::Streaming && e.uninterruptable {
                    self.stop_all_speech(device);
                }
                let cur = if info.sound_type == AudioType::Music { self.v.music } else { self.v.speech } * e.get_volume();
                let mut found = false;
                if handle_to_kill != 0 {
                    if let Some(i) = self.playing_streams.iter().position(|p| p.event.playing_handle == handle_to_kill) {
                        let p = self.playing_streams.remove(i);
                        self.release_playing_audio(device, p);
                        found = true;
                    }
                }
                let stream = if handle_to_kill == 0 || found {
                    device.start(&e.filename_to_load, VoiceStart { gain: cur, pan: 0.0, rate: 1.0, kind: VoiceKind::Stream })
                } else {
                    None
                };
                if stream.is_some() {
                    if info.sound_type == AudioType::Streaming && e.uninterruptable {
                        self.disallow_speech = true;
                    }
                    self.playing_streams.push(PlayingAudio::new(e, PlayingKind::Stream(stream), stream));
                }
            }
            AudioType::SoundEffect => {
                let three_d = e.is_positional_audio();
                let mut found = false;
                if handle_to_kill != 0 {
                    let list = if three_d { &mut self.playing_3d_sounds } else { &mut self.playing_sounds };
                    if let Some(i) = list.iter().position(|p| p.event.playing_handle == handle_to_kill) {
                        let p = list.remove(i);
                        self.release_playing_audio(device, p);
                        found = true;
                    }
                }
                let mut slot = None;
                if handle_to_kill == 0 || found {
                    slot = self.take_sample(three_d);
                    if slot.is_none() && self.kill_lowest_priority_sound_immediately(device, &e) {
                        slot = self.take_sample(three_d);
                    }
                }
                let Some(slot) = slot else { return };
                let voice = if three_d { self.play_sample_3d(host, device, &mut e) } else { self.play_sample(device, &e) };
                // the slot counts as started (`notifyOf*SampleStart`) whether or not the file played
                if three_d {
                    self.num_playing_3d += 1;
                } else {
                    self.num_playing_2d += 1;
                }
                let kind = if three_d { PlayingKind::Sample3D(Some(slot)) } else { PlayingKind::Sample(Some(slot)) };
                let p = PlayingAudio::new(e, kind, voice);
                if voice.is_none() {
                    // `releasePlayingAudio(audio)` at the end: the slot back and the count down
                    self.release_playing_audio(device, p);
                } else if three_d {
                    self.playing_3d_sounds.push(p);
                } else {
                    self.playing_sounds.push(p);
                }
            }
        }
    }

    /// `getFirst2DSample` / `getFirst3DSample`: the first free slot.
    fn take_sample(&mut self, three_d: bool) -> Option<u32> {
        let pool = if three_d { &mut self.available_3d_samples } else { &mut self.available_samples };
        (!pool.is_empty()).then(|| pool.remove(0))
    }

    /// `stopAudioEvent(handle)`: the music's special handles stop (or fade) the music stream; a stream of the handle
    /// ends at once; a sample of it is asked to stop (it ends when it next would loop).
    fn stop_audio_event<D: AudioDevice>(&mut self, host: &dyn AudioHost, device: &mut D, handle: AudioHandle) {
        if handle == AHSV_STOP_THE_MUSIC || handle == AHSV_STOP_THE_MUSIC_FADE {
            if let Some(i) = self.playing_streams.iter().position(|p| p.event.info().sound_type == AudioType::Music) {
                let p = self.playing_streams.remove(i);
                if handle == AHSV_STOP_THE_MUSIC_FADE {
                    self.fading_audio.push(p);
                } else {
                    self.release_playing_audio(device, p);
                }
            }
        }
        if let Some(i) = self.playing_streams.iter().position(|p| p.event.playing_handle == handle) {
            self.playing_streams[i].request_stop = true;
            if let Some(v) = self.playing_streams[i].voice {
                self.notify_of_audio_completion(host, device, v);
            }
        }
        for list in [&mut self.playing_sounds, &mut self.playing_3d_sounds] {
            if let Some(p) = list.iter_mut().find(|p| p.event.playing_handle == handle) {
                p.request_stop = true;
            }
        }
    }

    /// `killAudioEventImmediately(handle)`: a play request of it (upstream compares the request's handle field, which
    /// for a play request is the event pointer's bits, so it never matches; kept), else the playing sound.
    pub fn kill_audio_event_immediately<D: AudioDevice>(&mut self, device: &mut D, handle: AudioHandle) {
        for list in [0, 1, 2] {
            let l = match list {
                0 => &mut self.playing_3d_sounds,
                1 => &mut self.playing_sounds,
                _ => &mut self.playing_streams,
            };
            if let Some(i) = l.iter().position(|p| p.event.playing_handle == handle) {
                let p = l.remove(i);
                self.release_playing_audio(device, p);
                return;
            }
        }
    }

    /// `pauseAudio(which)`: the playing voices of those kinds paused; every play request dropped.
    pub fn pause_audio<D: AudioDevice>(&mut self, device: &mut D, which: u32) {
        self.set_paused(device, which, true);
        self.requests.retain(|r| r.request != RequestType::Play);
    }

    /// `resumeAudio(which)`.
    pub fn resume_audio<D: AudioDevice>(&mut self, device: &mut D, which: u32) {
        self.set_paused(device, which, false);
    }

    fn set_paused<D: AudioDevice>(&self, device: &mut D, which: u32, paused: bool) {
        let mut each = |l: &Vec<PlayingAudio>, ok: &dyn Fn(&PlayingAudio) -> bool| {
            for p in l.iter().filter(|p| ok(p)) {
                if let Some(v) = p.voice {
                    device.set_paused(v, paused);
                }
            }
        };
        if which & AUDIO_AFFECT_SOUND != 0 {
            each(&self.playing_sounds, &|_| true);
        }
        if which & AUDIO_AFFECT_SOUND_3D != 0 {
            each(&self.playing_3d_sounds, &|_| true);
        }
        if which & (AUDIO_AFFECT_SPEECH | AUDIO_AFFECT_MUSIC) != 0 {
            each(&self.playing_streams, &|p| {
                if p.event.info().sound_type == AudioType::Music {
                    which & AUDIO_AFFECT_MUSIC != 0
                } else {
                    which & AUDIO_AFFECT_SPEECH != 0
                }
            });
        }
    }

    /// `isCurrentlyPlaying(handle)`: playing, or requested.
    pub fn is_currently_playing(&self, handle: AudioHandle) -> bool {
        self.playing_sounds.iter().chain(&self.playing_3d_sounds).chain(&self.playing_streams).any(|p| p.event.playing_handle == handle)
            || self.requests.iter().any(|r| matches!(&r.pending, Ok(e) if e.playing_handle == handle))
    }

    /// `isMusicPlaying()`.
    pub fn is_music_playing(&self) -> bool {
        self.playing_streams.iter().any(|p| p.event.info().sound_type == AudioType::Music)
    }

    /// `hasMusicTrackCompleted(name, times)`.
    pub fn has_music_track_completed(&self, name: &[u8], times: i32) -> bool {
        self.playing_streams.iter().any(|p| p.event.info().sound_type == AudioType::Music && p.event.event_name == name && p.loops_completed >= times)
    }

    /// `getMusicTrackName()`: a music play request's, else the playing music's.
    pub fn get_music_track_name(&self) -> Vec<u8> {
        for r in &self.requests {
            if let (RequestType::Play, Ok(e)) = (r.request, &r.pending) {
                if e.info().sound_type == AudioType::Music {
                    return e.event_name.clone();
                }
            }
        }
        self.playing_streams.iter().find(|p| p.event.info().sound_type == AudioType::Music).map(|p| p.event.event_name.clone()).unwrap_or_default()
    }

    /// `nextMusicTrack()` (`prev`: `prevMusicTrack()`): the music stopped and the next track in the list added.
    pub fn next_music_track<D: AudioDevice>(&mut self, host: &dyn AudioHost, device: &mut D, prev: bool) {
        let mut name = Vec::new();
        for p in &self.playing_streams {
            if p.event.info().sound_type == AudioType::Music {
                name = p.event.event_name.clone();
            }
        }
        self.remove_audio_event(device, AHSV_STOP_THE_MUSIC);
        let name = if prev { self.tables.prev_track_name(&name) } else { self.tables.next_track_name(&name) };
        let mut t = AudioEventRts::new(&name);
        self.add_audio_event(host, &mut t);
    }

    /// `notifyOfAudioCompletion(voice)` (Miles' end callbacks): a loop goes round again, the attack and sound portions
    /// go on to the next, music restarts; anything else is stopped and cleaned up on the next frame.
    fn notify_of_audio_completion<D: AudioDevice>(&mut self, host: &dyn AudioHost, device: &mut D, voice: VoiceId) {
        let Some((list, i)) = self.find_playing_audio_from(voice) else { return };
        let mut p = self.list_mut(list)[i].clone();
        if self.disallow_speech && p.event.info().sound_type == AudioType::Streaming {
            self.disallow_speech = false;
        }
        let finished = (|| {
            if p.event.info().control & AC_LOOP != 0 {
                if p.event.portion_to_play_next == PortionToPlay::Attack {
                    p.event.portion_to_play_next = PortionToPlay::Sound;
                }
                if p.event.portion_to_play_next == PortionToPlay::Sound {
                    p.event.decrease_loop_count();
                    if self.start_next_loop(host, device, &mut p) {
                        return false;
                    }
                }
            }
            p.event.advance_next_play_portion();
            if p.event.portion_to_play_next != PortionToPlay::Done {
                match p.kind {
                    PlayingKind::Sample(_) => {
                        p.voice = self.play_sample(device, &p.event);
                        if p.voice.is_some() {
                            return false;
                        }
                    }
                    PlayingKind::Sample3D(_) => {
                        p.voice = self.play_sample_3d(host, device, &mut p.event);
                        if p.voice.is_some() {
                            return false;
                        }
                    }
                    PlayingKind::Stream(_) => {}
                }
            }
            if matches!(p.kind, PlayingKind::Stream(_)) && p.event.info().sound_type == AudioType::Music {
                // playStream again (see the module docs: Miles' infinite loop)
                p.loops_completed += 1;
                let gain = self.v.music * p.event.get_volume() * p.event.volume_shift;
                p.voice = device.start(&p.event.filename_to_load, VoiceStart { gain, pan: 0.0, rate: 1.0, kind: VoiceKind::Stream });
                if p.voice.is_some() {
                    return false;
                }
            }
            true
        })();
        if finished {
            p.stopped = true;
        }
        self.list_mut(list)[i] = p;
    }

    fn list_mut(&mut self, list: usize) -> &mut Vec<PlayingAudio> {
        match list {
            0 => &mut self.playing_sounds,
            1 => &mut self.playing_3d_sounds,
            _ => &mut self.playing_streams,
        }
    }

    /// `findPlayingAudioFrom(handle, kind)`: the list and index of the voice.
    fn find_playing_audio_from(&self, voice: VoiceId) -> Option<(usize, usize)> {
        for (li, l) in [&self.playing_sounds, &self.playing_3d_sounds, &self.playing_streams].into_iter().enumerate() {
            if let Some(i) = l.iter().position(|p| p.voice == Some(voice)) {
                return Some((li, i));
            }
        }
        None
    }

    /// `startNextLoop(looping)`: the next loop's file; with a delay of more than a frame the event goes back to the
    /// requests (this one stopped without cleaning the event up).
    fn start_next_loop<D: AudioDevice>(&mut self, host: &dyn AudioHost, device: &mut D, p: &mut PlayingAudio) -> bool {
        p.voice = None;
        if p.request_stop {
            return false;
        }
        if !p.event.has_more_loops() {
            return false;
        }
        p.event.generate_filename(&self.tables.settings, host, &mut self.random);
        if p.event.delay > MSEC_PER_LOGICFRAME_REAL {
            p.cleanup = false;
            p.request_stop = true;
            p.stopped = true;
            self.requests.push(AudioRequest { request: RequestType::Play, pending: Ok(p.event.clone()), requires_check_for_sample: true });
            return true;
        }
        p.voice = match p.kind {
            PlayingKind::Sample3D(_) => self.play_sample_3d(host, device, &mut p.event),
            _ => self.play_sample(device, &p.event),
        };
        p.voice.is_some()
    }

    /// `adjustVolumeOfPlayingAudio(name, volume)`: the override set on every playing event of the name, its voice at
    /// `volume * volumeShift` (upstream sets the raw product, not times the system volume).
    pub fn adjust_volume_of_playing_audio(&mut self, name: &[u8], volume: f32) {
        for l in [&mut self.playing_sounds, &mut self.playing_3d_sounds, &mut self.playing_streams] {
            for p in l.iter_mut().filter(|p| p.event.event_name == name) {
                p.event.volume = volume;
            }
        }
    }

    /// `removePlayingAudio(name)`.
    pub fn remove_playing_audio<D: AudioDevice>(&mut self, device: &mut D, name: &[u8]) {
        self.remove_where(device, |p| p.event.event_name == name);
    }

    /// `removeAllDisabledAudio()`: every playing event of volume 0.
    pub fn remove_all_disabled_audio<D: AudioDevice>(&mut self, device: &mut D) {
        self.remove_where(device, |p| p.event.get_volume() == 0.0);
    }

    fn remove_where<D: AudioDevice>(&mut self, device: &mut D, f: impl Fn(&PlayingAudio) -> bool) {
        for li in 0..3 {
            let (gone, keep): (Vec<_>, Vec<_>) = std::mem::take(self.list_mut(li)).into_iter().partition(|p| f(p));
            *self.list_mut(li) = keep;
            for p in gone {
                self.release_playing_audio(device, p);
            }
        }
    }

    /// `processRequestList()`: in order, requests appended meanwhile included; a request whose delay is a frame or more
    /// waits (`adjustRequest`), the others are processed (after `checkForSample` when they waited) and removed.
    fn process_request_list<D: AudioDevice>(&mut self, host: &dyn AudioHost, device: &mut D) {
        let mut i = 0;
        while i < self.requests.len() {
            // shouldProcessRequestThisFrame
            let process = match &self.requests[i].pending {
                Err(_) => true,
                Ok(e) => e.delay < MSEC_PER_LOGICFRAME_REAL,
            };
            if !process {
                // adjustRequest
                let r = &mut self.requests[i];
                if let Ok(e) = &mut r.pending {
                    e.decrement_delay(MSEC_PER_LOGICFRAME_REAL);
                    r.requires_check_for_sample = true;
                }
                i += 1;
                continue;
            }
            // checkForSample runs while the request is still listed (its own name counts towards the limit, as upstream)
            let go = !self.requests[i].requires_check_for_sample || {
                let mut r = self.requests[i].clone();
                let ok = self.check_for_sample(host, &mut r);
                self.requests[i] = r;
                ok
            };
            let req = self.requests.remove(i);
            if go {
                self.process_request(host, device, req);
            }
        }
    }

    /// `checkForSample(req)` (see the module docs for its comparison).
    fn check_for_sample(&self, host: &dyn AudioHost, req: &mut AudioRequest) -> bool {
        let Ok(e) = &mut req.pending else { return true };
        if e.event_info.is_none() {
            self.get_info_for_audio_event(e);
        }
        if e.info().sound_type_bits != AudioType::SoundEffect as u32 {
            return true;
        }
        self.can_play_now(host, e)
    }

    /// `processRequest(req)`.
    fn process_request<D: AudioDevice>(&mut self, host: &dyn AudioHost, device: &mut D, req: AudioRequest) {
        match (req.request, req.pending) {
            (RequestType::Play, Ok(e)) => self.play_audio_event(host, device, e),
            (RequestType::Stop, Err(h)) => self.stop_audio_event(host, device, h),
            _ => {}
        }
    }

    /// `processPlayingList()`: the stopped are released; the volume follows a change; a 3-D sound follows its owner,
    /// stops when the owner is dead, and is dropped when quieter than `MinSampleVolume` (unless `GLOBAL` or
    /// `CRITICAL`).
    fn process_playing_list<D: AudioDevice>(&mut self, host: &dyn AudioHost, device: &mut D) {
        let changed = self.volume_has_changed;
        for li in [0usize, 2] {
            let list = std::mem::take(self.list_mut(li));
            let mut keep = Vec::new();
            for p in list {
                if p.stopped {
                    self.release_playing_audio(device, p);
                } else {
                    if changed {
                        self.adjust_playing_volume(device, &p, 0.0);
                    }
                    keep.push(p);
                }
            }
            *self.list_mut(li) = keep;
        }
        let list = std::mem::take(&mut self.playing_3d_sounds);
        let mut keep = Vec::new();
        let mut stop_handles = Vec::new();
        for mut p in list {
            if p.stopped {
                self.release_playing_audio(device, p);
                continue;
            }
            let pos = if p.event.is_positional_audio() { p.event.get_current_position(host) } else { None };
            let Some(pos) = pos else {
                self.release_playing_audio(device, p);
                continue;
            };
            if p.event.is_dead() {
                stop_handles.push(p.event.playing_handle);
                keep.push(p);
                continue;
            }
            let mut vol = self.get_effective_volume(host, &mut p.event);
            vol /= if self.v.sound_3d > 0.0 { self.v.sound } else { 1.0 };
            let info = p.event.info();
            let play_anyway = info.sound_type_bits & ST_GLOBAL != 0 || info.priority == AudioPriority::Critical;
            if vol < self.tables.settings.min_volume && !play_anyway {
                self.release_playing_audio(device, p);
                continue;
            }
            // AIL_set_3D_position: here the voice's gain and pan
            if let Some(v) = p.voice {
                let gain = self.get_effective_volume(host, &mut p.event);
                device.set_gain_pan(v, gain, self.pan_of(&pos));
            }
            keep.push(p);
        }
        self.playing_3d_sounds = keep;
        for h in stop_handles {
            self.stop_audio_event(host, device, h);
        }
        self.volume_has_changed = false;
    }

    /// `has3DSensitiveStreamsPlaying()`: a stream that is not music, or music not named `Game_*`.
    pub fn has_3d_sensitive_streams_playing(&self) -> bool {
        self.playing_streams.iter().any(|p| p.event.info().sound_type != AudioType::Music || !p.event.event_name.starts_with(b"Game_"))
    }

    /// `processFadingList()`: each fades over `TimeToFadeAudio` frames, then is released.
    fn process_fading_list<D: AudioDevice>(&mut self, device: &mut D) {
        let frames = self.tables.settings.fade_audio_frames;
        let list = std::mem::take(&mut self.fading_audio);
        let mut keep = Vec::new();
        for mut p in list {
            if p.frames_faded >= frames {
                self.release_playing_audio(device, p);
                continue;
            }
            p.frames_faded += 1;
            // getEffectiveVolume of a stream (no position needed)
            let base = p.event.get_volume() * p.event.volume_shift * if p.event.info().sound_type == AudioType::Music { self.v.music } else { self.v.speech };
            let vol = base * (1.0f32 - 1.0f32 * p.frames_faded as f32 / frames as f32);
            if let Some(v) = p.voice {
                device.set_gain_pan(v, vol, 0.0);
            }
            keep.push(p);
        }
        self.fading_audio = keep;
    }

    /// `getEffectiveVolume(event)` (see the module docs).
    pub fn get_effective_volume(&self, host: &dyn AudioHost, e: &mut AudioEventRts) -> f32 {
        let mut volume = 1.0f32 * (e.get_volume() * e.volume_shift);
        let info = e.event_info.clone().expect("described");
        match info.sound_type {
            AudioType::Music => volume *= self.v.music,
            AudioType::Streaming => volume *= self.v.speech,
            AudioType::SoundEffect => {
                if e.is_positional_audio() {
                    volume *= self.v.sound_3d;
                    if let Some(pos) = e.get_current_position(host) {
                        let d = Coord3D { x: self.listener_position.x - pos.x, y: self.listener_position.y - pos.y, z: self.listener_position.z - pos.z };
                        let (min, max) = if info.sound_type_bits & ST_GLOBAL != 0 {
                            (self.tables.settings.global_min_range as f32, self.tables.settings.global_max_range as f32)
                        } else {
                            (info.min_distance, info.max_distance)
                        };
                        let dist = d.length();
                        if dist > min {
                            volume *= 1.0 / (dist / min);
                        }
                        if dist >= max {
                            volume = 0.0;
                        }
                    }
                } else {
                    volume *= self.v.sound;
                }
            }
        }
        volume
    }

    /// The voices of every playing sound (for checks).
    pub fn playing_count(&self) -> (usize, usize, usize) {
        (self.playing_sounds.len(), self.playing_3d_sounds.len(), self.playing_streams.len())
    }
}

impl PlayingAudio {
    /// `allocatePlayingAudio()` with its fields set.
    fn new(event: Box<AudioEventRts>, kind: PlayingKind, voice: Option<VoiceId>) -> Self {
        PlayingAudio { event, kind, voice, stopped: false, request_stop: false, frames_faded: 0, cleanup: true, loops_completed: 0 }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::client::audio_event::tests::TestHost;

    /// A device that records what it is asked: voices numbered from 1.
    #[derive(Default, Debug)]
    pub struct LogDevice {
        pub next: VoiceId,
        pub started: Vec<(VoiceId, Vec<u8>, VoiceStart)>,
        pub stopped: Vec<VoiceId>,
        pub gains: std::collections::BTreeMap<VoiceId, (f32, f32)>,
        pub finished: Vec<VoiceId>,
        pub missing: Vec<Vec<u8>>,
    }
    impl AudioDevice for LogDevice {
        fn start(&mut self, file: &[u8], s: VoiceStart) -> Option<VoiceId> {
            if self.missing.iter().any(|m| m == file) {
                return None;
            }
            self.next += 1;
            self.started.push((self.next, file.to_vec(), s));
            self.gains.insert(self.next, (s.gain, s.pan));
            Some(self.next)
        }
        fn set_gain_pan(&mut self, v: VoiceId, g: f32, p: f32) {
            self.gains.insert(v, (g, p));
        }
        fn stop(&mut self, v: VoiceId) {
            self.stopped.push(v);
        }
        fn set_paused(&mut self, _: VoiceId, _: bool) {}
        fn take_finished(&mut self) -> Vec<VoiceId> {
            std::mem::take(&mut self.finished)
        }
        fn file_length_ms(&self, _: &[u8]) -> f32 {
            1000.0
        }
    }

    /// Players 0 (local) and 1 (its enemy) and 2 (its ally); the shroud clear but at x < 0.
    impl AudioHost for TestHost {
        fn local_player_index(&self) -> Option<i32> {
            Some(0)
        }
        fn listening_player_index(&self) -> Option<i32> {
            Some(0)
        }
        fn player_exists(&self, i: i32) -> bool {
            (0..3).contains(&i)
        }
        fn player_has_default_team(&self, _: i32) -> bool {
            true
        }
        fn are_allies(&self, o: i32, l: i32) -> Option<bool> {
            Some(o == l || o == 2)
        }
        fn shroud_clear(&self, _: i32, pos: &Coord3D) -> bool {
            pos.x >= 0.0
        }
        fn ground_height(&self, _: f32, _: f32) -> f32 {
            10.0
        }
    }

    pub fn manager() -> AudioManager {
        let mut t = AudioTables::new();
        t.load_text(
            b"t.ini",
            b"AudioSettings\n AudioRoot = Data\\Audio\n SoundsFolder = Sounds\n MusicFolder = Tracks\n StreamingFolder = Speech\n SoundsExtension = wav\n\
 SampleCount2D = 2\n SampleCount3D = 3\n MinSampleVolume = 5%\n GlobalMinRange = 200\n GlobalMaxRange = 1000\n TimeToFadeAudio = 100\n\
 DefaultSoundVolume = 100%\n Default3DSoundVolume = 100%\n DefaultSpeechVolume = 100%\n DefaultMusicVolume = 50%\n\
 MicrophoneDesiredHeightAboveTerrain = 100\n MicrophoneMaxPercentageBetweenGroundAndCamera = 50%\nEnd\n\
AudioEvent Gun\n Sounds = gun\n Type = world everyone\n MinRange = 100\n MaxRange = 500\n Volume = 100%\n Limit = 2\n Priority = normal\nEnd\n\
AudioEvent Fog\n Sounds = fog\n Type = world shrouded everyone\n MinRange = 100\n MaxRange = 500\n Volume = 100%\nEnd\n\
AudioEvent Zap\n Sounds = zap\n Type = world everyone\n MinRange = 100\n MaxRange = 500\n Volume = 100%\n Limit = 1\n Control = interrupt\nEnd\n\
AudioEvent Low\n Sounds = low\n Type = ui everyone\n Volume = 100%\n Priority = lowest\nEnd\n\
AudioEvent High\n Sounds = high\n Type = ui everyone\n Volume = 100%\n Priority = high\nEnd\n\
AudioEvent Mine\n Sounds = mine\n Type = ui player\n Volume = 100%\nEnd\n\
AudioEvent Friends\n Sounds = f\n Type = ui allies\n Volume = 100%\nEnd\n\
AudioEvent Quiet\n Sounds = q\n Type = ui everyone\n Volume = 1%\nEnd\n\
AudioEvent Loop\n Sounds = l1 l2\n Type = ui everyone\n Volume = 100%\n Control = loop\n LoopCount = 3\nEnd\n\
AudioEvent Late\n Sounds = late\n Type = ui everyone\n Volume = 100%\n Delay = 100 100\nEnd\n\
AudioEvent Says\n Sounds = says\n Type = world voice everyone\n Priority = high\n MinRange = 100\n MaxRange = 500\n Volume = 100%\nEnd\n\
MusicTrack DefaultMusicTrack\n Volume = 100%\nEnd\n\
MusicTrack Game_A\n Filename = a.mp3\nEnd\n\
MusicTrack Game_B\n Filename = b.mp3\nEnd\n",
        )
        .expect("parses");
        AudioManager::new(t)
    }

    fn at(name: &[u8], x: f32) -> AudioEventRts {
        AudioEventRts::at_position(name, &Coord3D { x, y: 0.0, z: 0.0 })
    }

    fn frame(m: &mut AudioManager, h: &TestHost, d: &mut LogDevice) {
        m.update(h, d, &ListenerView::default());
    }

    /// Predicted, the microphone at (0, 0, 10) (ground height 10, the camera at the ground point): a positional sound at
    /// 300 plays at 100/300 of full volume; at 600 (beyond its 500) it is refused; a shrouded sound at x = -300 is
    /// refused; `Mine` plays for player 0 and not for 1; `Friends` plays for the ally 2 and not for the local player
    /// itself; `Quiet` (1 % < 5 %) is muted; the handles count up from 6.
    #[test]
    fn culling_by_range_shroud_owner_and_volume() {
        let mut m = manager();
        let h = TestHost::default();
        let mut d = LogDevice::default();
        frame(&mut m, &h, &mut d);
        assert_eq!(m.listener_position, Coord3D { x: 0.0, y: 0.0, z: 10.0 });
        let h1 = m.add_audio_event(&h, &mut at(b"Gun", 300.0));
        assert_eq!(h1, AHSV_FIRST_HANDLE);
        frame(&mut m, &h, &mut d);
        assert_eq!(d.started.len(), 1);
        let (v, ref file, s) = d.started[0];
        assert_eq!(file, b"Data\\Audio\\Sounds\\gun.wav");
        let dist = Coord3D { x: 300.0, y: 0.0, z: -10.0 }.length();
        assert!((d.gains[&v].0 - 100.0 / dist).abs() < 1e-6, "{:?}", d.gains[&v]);
        assert_eq!(s.kind, VoiceKind::Sample3D);
        assert!(d.gains[&v].1 > 0.99, "east of a listener facing north is right");
        m.add_audio_event(&h, &mut at(b"Gun", 600.0));
        m.add_audio_event(&h, &mut at(b"Fog", -300.0));
        assert_eq!(m.requests.len(), 0);
        let mut mine = AudioEventRts::new(b"Mine");
        mine.player_index = 0;
        assert!(m.add_audio_event(&h, &mut mine) >= AHSV_FIRST_HANDLE);
        mine.player_index = 1;
        assert_eq!(m.add_audio_event(&h, &mut mine), AHSV_NOT_FOR_LOCAL);
        let mut f = AudioEventRts::new(b"Friends");
        f.player_index = 2;
        assert!(m.add_audio_event(&h, &mut f) >= AHSV_FIRST_HANDLE);
        f.player_index = 0;
        assert_eq!(m.add_audio_event(&h, &mut f), AHSV_NOT_FOR_LOCAL);
        assert_eq!(m.add_audio_event(&h, &mut AudioEventRts::new(b"Quiet")), AHSV_MUTED);
        assert_eq!(m.add_audio_event(&h, &mut AudioEventRts::new(b"Nope")), AHSV_ERROR);
        assert_eq!(m.add_audio_event(&h, &mut AudioEventRts::new(b"NoSound")), AHSV_NO_SOUND);
    }

    /// Predicted: `Gun` (limit 2) plays twice and a third is refused while both play; `Zap` (limit 1, interrupt)
    /// replaces the playing one, whose voice is stopped; two 2-D slots: a third 2-D sound of normal priority is
    /// refused... unless a lower-priority one plays: `High` takes `Low`'s slot. A voice sound of an object refuses a
    /// second while the first plays.
    #[test]
    fn limits_interrupts_priorities_and_voices() {
        let mut m = manager();
        let h = TestHost::default();
        let mut d = LogDevice::default();
        frame(&mut m, &h, &mut d);
        for _ in 0..3 {
            m.add_audio_event(&h, &mut at(b"Gun", 100.0));
        }
        frame(&mut m, &h, &mut d);
        assert_eq!(m.playing_3d_sounds.len(), 2, "the third over the limit (two requested in the same frame)");
        m.add_audio_event(&h, &mut at(b"Zap", 100.0));
        frame(&mut m, &h, &mut d);
        let first_zap = m.playing_3d_sounds.iter().find(|p| p.event.event_name == b"Zap").unwrap().voice.unwrap();
        m.add_audio_event(&h, &mut at(b"Zap", 100.0));
        frame(&mut m, &h, &mut d);
        assert_eq!(m.playing_3d_sounds.iter().filter(|p| p.event.event_name == b"Zap").count(), 1);
        assert!(d.stopped.contains(&first_zap), "the old Zap's voice stopped");
        // 2-D: two slots
        m.add_audio_event(&h, &mut AudioEventRts::new(b"Low"));
        m.add_audio_event(&h, &mut AudioEventRts::new(b"Low"));
        frame(&mut m, &h, &mut d);
        assert_eq!(m.playing_sounds.len(), 2);
        assert_eq!(m.add_audio_event(&h, &mut AudioEventRts::new(b"Low")), m.handle_pool - 1);
        assert!(m.requests.is_empty(), "no slot and nothing lower: refused");
        m.add_audio_event(&h, &mut AudioEventRts::new(b"High"));
        frame(&mut m, &h, &mut d);
        let names: Vec<_> = m.playing_sounds.iter().map(|p| p.event.event_name.clone()).collect();
        assert_eq!(names, [b"Low".to_vec(), b"High".to_vec()]);
        assert_eq!(m.num_playing_2d, 2);
        // voices
        let mut h2 = TestHost::default();
        h2.objects.push((7, Coord3D { x: 50.0, y: 0.0, z: 0.0 }, 0));
        m.add_audio_event(&h2, &mut AudioEventRts::with_object(b"Says", 7));
        frame(&mut m, &h2, &mut d);
        assert!(m.is_object_playing_voice(7));
        m.add_audio_event(&h2, &mut AudioEventRts::with_object(b"Says", 7));
        assert!(m.requests.is_empty());
    }

    /// Predicted: `Loop` (loop count 3) plays its two files in turn three times then stops: l1, l2, l1, and the slot is
    /// back; `Late` (delay 100 ms) ~~waits three frames~~ starts on the fourth frame (**missed**, 2026-11-06: in `float`,
    /// 100 - 2 * 33.333332 = 33.333336 is not under one frame, 33.333332, so it waits once more, as upstream's would).
    #[test]
    fn loops_and_delays() {
        let mut m = manager();
        let h = TestHost::default();
        let mut d = LogDevice::default();
        m.add_audio_event(&h, &mut AudioEventRts::new(b"Loop"));
        frame(&mut m, &h, &mut d);
        for _ in 0..4 {
            let v = d.next;
            d.finished.push(v);
            frame(&mut m, &h, &mut d);
        }
        let files: Vec<_> = d.started.iter().map(|s| String::from_utf8(s.1.clone()).unwrap()).collect();
        assert_eq!(files, ["Data\\Audio\\Sounds\\l1.wav", "Data\\Audio\\Sounds\\l2.wav", "Data\\Audio\\Sounds\\l1.wav"]);
        assert!(m.playing_sounds.is_empty());
        assert_eq!(m.available_samples.len(), 2);
        assert_eq!(m.num_playing_2d, 0);
        let n = d.started.len();
        m.add_audio_event(&h, &mut AudioEventRts::new(b"Late"));
        let mut frames = 0;
        while d.started.len() == n {
            frame(&mut m, &h, &mut d);
            frames += 1;
            assert!(frames < 10);
        }
        assert_eq!(frames, 4);
    }

    /// Predicted: music plays as a stream at music volume 0.5, restarts when it ends (one loop completed), the next track
    /// after `Game_A` is `Game_B`, and a fading stop takes 100 ms = 3 frames of falling gain before the voice stops.
    #[test]
    fn music_tracks_loop_and_fade() {
        let mut m = manager();
        let h = TestHost::default();
        let mut d = LogDevice::default();
        m.add_audio_event(&h, &mut AudioEventRts::new(b"Game_A"));
        assert_eq!(m.get_music_track_name(), b"Game_A");
        frame(&mut m, &h, &mut d);
        assert!(m.is_music_playing());
        assert_eq!(d.started[0].1, b"Data\\Audio\\Tracks\\a.mp3");
        assert_eq!(d.started[0].2.gain, 0.5);
        d.finished.push(d.next);
        frame(&mut m, &h, &mut d);
        assert!(m.has_music_track_completed(b"Game_A", 1));
        assert!(!m.has_music_track_completed(b"Game_A", 2));
        m.next_music_track(&h, &mut d, false);
        frame(&mut m, &h, &mut d);
        assert_eq!(m.get_music_track_name(), b"Game_B");
        let v = d.next;
        m.remove_audio_event(&mut d, AHSV_STOP_THE_MUSIC_FADE);
        let mut gains = Vec::new();
        for _ in 0..5 {
            frame(&mut m, &h, &mut d);
            gains.push(d.gains[&v].0);
        }
        assert!(gains[0] < 0.5 && gains[1] < gains[0] && gains[2] == 0.0, "{gains:?}");
        assert!(d.stopped.contains(&v));
        assert!(!m.is_music_playing());
    }

    /// Predicted: script volumes multiply the system ones; the zoom boost: with 25 % and the camera within the minimum
    /// distance, the 3-D volume is the full system volume; a volume override of 0 then `removeDisabledEvents` removes the
    /// playing sound.
    #[test]
    fn volumes_overrides_and_zoom() {
        let mut m = manager();
        let h = TestHost::default();
        let mut d = LogDevice::default();
        m.set_volume(0.5, AUDIO_AFFECT_MUSIC);
        assert_eq!(m.get_volume(AUDIO_AFFECT_MUSIC), 0.25);
        m.set_volume(0.8, AUDIO_AFFECT_SOUND | AUDIO_AFFECT_SYSTEM_SETTING);
        assert_eq!(m.get_volume(AUDIO_AFFECT_SOUND), 0.8);
        m.tables.settings.zoom_sound_volume_percentage_amount = 0.25;
        m.tables.settings.zoom_min_distance = 50.0;
        m.tables.settings.zoom_max_distance = 150.0;
        m.update(&h, &mut d, &ListenerView { ground: Coord3D::default(), angle: 0.0, camera: Coord3D { x: 0.0, y: 0.0, z: 60.0 } });
        // the microphone: ground 10, then half way to the camera (60 <= 100 desired height): z = 10 + 30
        assert_eq!(m.listener_position.z, 40.0);
        assert_eq!(m.v.zoom, 1.0);
        m.update(&h, &mut d, &ListenerView { ground: Coord3D::default(), angle: 0.0, camera: Coord3D { x: 0.0, y: 0.0, z: 1000.0 } });
        assert_eq!(m.v.zoom, 0.75);
        assert_eq!(m.v.sound_3d, 0.75);
        m.add_audio_event(&h, &mut AudioEventRts::new(b"High"));
        frame(&mut m, &h, &mut d);
        assert_eq!(m.playing_sounds.len(), 1);
        m.set_audio_event_enabled(b"High", false);
        m.remove_disabled_events(&mut d);
        assert!(m.playing_sounds.is_empty());
        assert_eq!(m.add_audio_event(&h, &mut AudioEventRts::new(b"High")), AHSV_MUTED, "the override applies to new events");
        m.set_audio_event_enabled(b"High", true);
        assert!(m.adjusted_volumes.is_empty());
    }
}
