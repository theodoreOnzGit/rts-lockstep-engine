// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/AudioEventInfo.h (AudioType, AudioPriority, SoundType, AudioControl,
//     struct AudioEventInfo, isPermanentSound)
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INIAudioEventInfo.cpp (INI::parseMusicTrackDefinition,
//     INI::parseAudioEventDefinition, INI::parseDialogDefinition, AudioEventInfo::m_audioEventInfo, parseDelay,
//     parsePitchShift, theAudioPriorityNames, theSoundTypeNames, theAudioControlNames)
//   GeneralsMD/Code/GameEngine/Include/Common/AudioSettings.h (struct AudioSettings)
//   GeneralsMD/Code/GameEngine/Source/Common/Audio/GameAudio.cpp (audioSettingsFieldParseTable, TheSpeakerTypes,
//     parseSpeakerType, AudioManager::translateSpeakerTypeToUnsignedInt, translateUnsignedIntToSpeakerType,
//     INI::parseAudioSettingsDefinition, AudioManager::init's file list, newAudioEventInfo, findAudioEventInfo,
//     addTrackName, nextTrackName, prevTrackName, findAllAudioEventsOfType)
//   GeneralsMD/Code/GameEngine/Include/Common/MiscAudio.h, Source/Common/INI/INIMiscAudio.cpp (MiscAudio,
//     MiscAudio::m_fieldParseTable, INI::parseMiscAudio)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/OptionsMenu.cpp (OptionPreferences::
//     getSoundVolume, get3DSoundVolume, getSpeechVolume, getMusicVolume, getPreferred3DProvider, getSpeakerType)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The audio's static data (GitHub #13, C6): every audio event's description from `SoundEffects.ini`, `Voice.ini`,
//! `Speech.ini` and `Music.ini`, the audio settings and the miscellaneous sounds.
//!
//! # As upstream does it
//!
//! `AudioManager::init` loads, in order, `AudioSettings.ini`, then each of `Music.ini`, `SoundEffects.ini`, `Speech.ini`
//! and `Voice.ini` from `Data\INI\Default\` and then `Data\INI\`, and `MiscAudio.ini` last. An `AudioEvent` block starts
//! as a copy of `DefaultSoundEffect` (if one was defined before it), a `DialogEvent` of `DefaultDialog`, a `MusicTrack`
//! of `DefaultMusicTrack`; a music track is added to the track list only when `DefaultMusicTrack` exists.
//!
//! # Where this differs from upstream
//!
//! - **The descriptions are shared with `Arc`** (upstream: pointers into the manager's hash map), so an
//!   [`crate::client::audio_event::AudioEventRts`] holds its own reference.
//! - **A new description starts zeroed**, as upstream's pooled `newInstance` (`MemoryPool::allocateBlock` zeroes).
//!   `AudioSettings` has no constructor upstream (its plain fields are left as `new` leaves them); here they are zero.
//! - **The user's `Options.ini`** (`OptionPreferences`) is an optional map of its keys ([`OptionPrefs`]); without it the
//!   preferred volumes come from the defaults, as upstream's getters do with no entry.
//! - `SoundsMorning`, `SoundsNight` and `SoundsEvening` are parsed and kept but, as upstream, never read:
//!   `AudioEventRTS::generateFilename` only ever picks from `Sounds`.
//! - The 3-D provider and speaker selection (`setPreferredProvider`, `setPreferredSpeaker`) belong to the Miles device,
//!   which was never released; the names are kept in [`AudioSettings`] and [`AudioTables::preferred_provider`].

use crate::common::audio_event_name::AudioEventName;
use crate::common::ini::{BlockParse, FieldParse, Ini, IniLoadType, IniResult, UserData};
use std::collections::BTreeMap;
use std::sync::Arc;

/// `enum AudioType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AudioType {
    /// `AT_Music` (0, a zeroed description's).
    #[default]
    Music = 0,
    /// `AT_Streaming`.
    Streaming = 1,
    /// `AT_SoundEffect`.
    SoundEffect = 2,
}

/// `enum AudioPriority` (`AP_LOWEST` .. `AP_CRITICAL`, 0 .. 4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AudioPriority {
    #[default]
    Lowest = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

impl AudioPriority {
    /// The enum of an index (`parseIndexList` stores the index; outside 0..=4 is not a name in the list).
    pub fn from_index(i: i32) -> AudioPriority {
        match i {
            0 => AudioPriority::Lowest,
            1 => AudioPriority::Low,
            2 => AudioPriority::Normal,
            3 => AudioPriority::High,
            _ => AudioPriority::Critical,
        }
    }
}

/// `theAudioPriorityNames`.
pub const AUDIO_PRIORITY_NAMES: &[&str] = &["LOWEST", "LOW", "NORMAL", "HIGH", "CRITICAL"];
/// `theSoundTypeNames` (the bits of `enum SoundType`, in order).
pub const SOUND_TYPE_NAMES: &[&str] = &["UI", "WORLD", "SHROUDED", "GLOBAL", "VOICE", "PLAYER", "ALLIES", "ENEMIES", "EVERYONE"];
/// `theAudioControlNames` (the bits of `enum AudioControl`, in order).
pub const AUDIO_CONTROL_NAMES: &[&str] = &["LOOP", "RANDOM", "ALL", "POSTDELAY", "INTERRUPT"];

/// `ST_UI`.
pub const ST_UI: u32 = 0x0001;
/// `ST_WORLD`.
pub const ST_WORLD: u32 = 0x0002;
/// `ST_SHROUDED`.
pub const ST_SHROUDED: u32 = 0x0004;
/// `ST_GLOBAL`.
pub const ST_GLOBAL: u32 = 0x0008;
/// `ST_VOICE`.
pub const ST_VOICE: u32 = 0x0010;
/// `ST_PLAYER`.
pub const ST_PLAYER: u32 = 0x0020;
/// `ST_ALLIES`.
pub const ST_ALLIES: u32 = 0x0040;
/// `ST_ENEMIES`.
pub const ST_ENEMIES: u32 = 0x0080;
/// `ST_EVERYONE`.
pub const ST_EVERYONE: u32 = 0x0100;

/// `AC_LOOP`.
pub const AC_LOOP: u32 = 0x0001;
/// `AC_RANDOM`.
pub const AC_RANDOM: u32 = 0x0002;
/// `AC_ALL`.
pub const AC_ALL: u32 = 0x0004;
/// `AC_POSTDELAY`.
pub const AC_POSTDELAY: u32 = 0x0008;
/// `AC_INTERRUPT`.
pub const AC_INTERRUPT: u32 = 0x0010;

/// `struct AudioEventInfo`: one audio event's description.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioEventInfo {
    /// `m_audioName`.
    pub audio_name: Vec<u8>,
    /// `m_filename` (music and speech: the file).
    pub filename: Vec<u8>,
    /// `m_volume`.
    pub volume: f32,
    /// `m_volumeShift`.
    pub volume_shift: f32,
    /// `m_minVolume`.
    pub min_volume: f32,
    /// `m_pitchShiftMin` (`1 + min/100`).
    pub pitch_shift_min: f32,
    /// `m_pitchShiftMax`.
    pub pitch_shift_max: f32,
    /// `m_delayMin` (ms).
    pub delay_min: i32,
    /// `m_delayMax` (ms).
    pub delay_max: i32,
    /// `m_limit`.
    pub limit: i32,
    /// `m_loopCount`.
    pub loop_count: i32,
    /// `m_priority`.
    pub priority: AudioPriority,
    /// `m_type`: [`SOUND_TYPE_NAMES`] bits.
    pub sound_type_bits: u32,
    /// `m_control`: [`AUDIO_CONTROL_NAMES`] bits.
    pub control: u32,
    /// `m_soundsMorning`.
    pub sounds_morning: Vec<Vec<u8>>,
    /// `m_sounds`.
    pub sounds: Vec<Vec<u8>>,
    /// `m_soundsNight`.
    pub sounds_night: Vec<Vec<u8>>,
    /// `m_soundsEvening`.
    pub sounds_evening: Vec<Vec<u8>>,
    /// `m_attackSounds`.
    pub attack_sounds: Vec<Vec<u8>>,
    /// `m_decaySounds`.
    pub decay_sounds: Vec<Vec<u8>>,
    /// `m_lowPassFreq`.
    pub low_pass_freq: f32,
    /// `m_minDistance`.
    pub min_distance: f32,
    /// `m_maxDistance`.
    pub max_distance: f32,
    /// `m_soundType`.
    pub sound_type: AudioType,
}

impl AudioEventInfo {
    /// `isPermanentSound()`: loops forever (`AC_LOOP` with a loop count of 0).
    pub fn is_permanent_sound(&self) -> bool {
        self.control & AC_LOOP != 0 && self.loop_count == 0
    }
}

/// `parseDelay`: two integers, min then max (upstream asserts `0 <= min <= max` in debug builds only).
fn parse_delay(ini: &mut Ini<'_>, a: &mut AudioEventInfo, _: &UserData) -> IniResult<()> {
    let min = Ini::scan_int(&ini.get_next_token(None)?)?;
    let max = Ini::scan_int(&ini.get_next_token(None)?)?;
    a.delay_max = max;
    a.delay_min = min;
    Ok(())
}

/// `parsePitchShift`: two percentages, stored as `1 + p/100`.
fn parse_pitch_shift(ini: &mut Ini<'_>, a: &mut AudioEventInfo, _: &UserData) -> IniResult<()> {
    let min = Ini::scan_real(&ini.get_next_token(None)?)?;
    let max = Ini::scan_real(&ini.get_next_token(None)?)?;
    a.pitch_shift_min = 1.0f32 + min / 100.0;
    a.pitch_shift_max = 1.0f32 + max / 100.0;
    Ok(())
}

/// `AudioEventInfo::m_audioEventInfo`.
pub static AUDIO_EVENT_INFO_FIELDS: [FieldParse<AudioEventInfo>; 21] = [
    FieldParse::new("Filename", |i, a, _| i.parse_ascii_string().map(|v| a.filename = v), UserData::None),
    FieldParse::new("Volume", |i, a, _| i.parse_percent_to_real().map(|v| a.volume = v), UserData::None),
    FieldParse::new("VolumeShift", |i, a, _| i.parse_percent_to_real().map(|v| a.volume_shift = v), UserData::None),
    FieldParse::new("MinVolume", |i, a, _| i.parse_percent_to_real().map(|v| a.min_volume = v), UserData::None),
    FieldParse::new("PitchShift", parse_pitch_shift, UserData::None),
    FieldParse::new("Delay", parse_delay, UserData::None),
    FieldParse::new("Limit", |i, a, _| i.parse_int().map(|v| a.limit = v), UserData::None),
    FieldParse::new("LoopCount", |i, a, _| i.parse_int().map(|v| a.loop_count = v), UserData::None),
    FieldParse::new("Priority", |i, a, u| i.parse_index_list(u.names()).map(|v| a.priority = AudioPriority::from_index(v)), UserData::Names(AUDIO_PRIORITY_NAMES)),
    FieldParse::new("Type", |i, a, u| i.parse_bit_string32(&mut a.sound_type_bits, u.names()), UserData::Names(SOUND_TYPE_NAMES)),
    FieldParse::new("Control", |i, a, u| i.parse_bit_string32(&mut a.control, u.names()), UserData::Names(AUDIO_CONTROL_NAMES)),
    FieldParse::new("Sounds", |i, a, _| i.parse_sounds_list(&mut a.sounds), UserData::None),
    FieldParse::new("SoundsNight", |i, a, _| i.parse_sounds_list(&mut a.sounds_night), UserData::None),
    FieldParse::new("SoundsEvening", |i, a, _| i.parse_sounds_list(&mut a.sounds_evening), UserData::None),
    FieldParse::new("SoundsMorning", |i, a, _| i.parse_sounds_list(&mut a.sounds_morning), UserData::None),
    FieldParse::new("Attack", |i, a, _| i.parse_sounds_list(&mut a.attack_sounds), UserData::None),
    FieldParse::new("Decay", |i, a, _| i.parse_sounds_list(&mut a.decay_sounds), UserData::None),
    FieldParse::new("MinRange", |i, a, _| i.parse_real().map(|v| a.min_distance = v), UserData::None),
    FieldParse::new("MaxRange", |i, a, _| i.parse_real().map(|v| a.max_distance = v), UserData::None),
    FieldParse::new("LowPassCutoff", |i, a, _| i.parse_percent_to_real().map(|v| a.low_pass_freq = v), UserData::None),
    FieldParse::end(),
];

/// `MAX_HW_PROVIDERS`.
pub const MAX_HW_PROVIDERS: usize = 4;

/// `TheSpeakerTypes`.
pub const SPEAKER_TYPES: &[&str] = &["2 Speakers", "Headphones", "Surround Sound", "4 Speaker", "5.1 Surround", "7.1 Surround"];

/// `AudioManager::translateSpeakerTypeToUnsignedInt`: the index of the name (exact case), 0 if it is none.
pub fn translate_speaker_type_to_unsigned_int(speaker_type: &[u8]) -> u32 {
    SPEAKER_TYPES.iter().position(|s| s.as_bytes() == speaker_type).map_or(0, |i| i as u32)
}

/// `AudioManager::translateUnsignedIntToSpeakerType`: the name, the first for an index out of range.
pub fn translate_unsigned_int_to_speaker_type(speaker_type: u32) -> &'static str {
    // upstream compares with `TheSpeakerTypesCount`, which counts the terminating NULL too: index 6 reads that NULL
    // (an empty string here)
    if speaker_type as usize > SPEAKER_TYPES.len() {
        return SPEAKER_TYPES[0];
    }
    SPEAKER_TYPES.get(speaker_type as usize).copied().unwrap_or("")
}

/// `struct AudioSettings`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioSettings {
    pub audio_root: Vec<u8>,
    pub sounds_folder: Vec<u8>,
    pub music_folder: Vec<u8>,
    pub streaming_folder: Vec<u8>,
    pub sounds_extension: Vec<u8>,
    pub use_digital: bool,
    pub use_midi: bool,
    pub output_rate: i32,
    pub output_bits: i32,
    pub output_channels: i32,
    pub sample_count_2d: i32,
    pub sample_count_3d: i32,
    pub stream_count: i32,
    pub global_min_range: i32,
    pub global_max_range: i32,
    /// `m_drawableAmbientFrames` (`TimeBetweenDrawableSounds`, frames).
    pub drawable_ambient_frames: u32,
    /// `m_fadeAudioFrames` (`TimeToFadeAudio`, frames).
    pub fade_audio_frames: u32,
    /// `m_maxCacheSize` (`AudioFootprintInBytes`).
    pub max_cache_size: u32,
    /// `m_minVolume` (`MinSampleVolume`): quieter samples are culled.
    pub min_volume: f32,
    /// `m_preferred3DProvider[MAX_HW_PROVIDERS + 1]`.
    pub preferred_3d_provider: [Vec<u8>; MAX_HW_PROVIDERS + 1],
    pub relative_2d_volume: f32,
    pub default_sound_volume: f32,
    pub default_3d_sound_volume: f32,
    pub default_speech_volume: f32,
    pub default_music_volume: f32,
    pub default_speaker_type_2d: u32,
    pub default_speaker_type_3d: u32,
    pub preferred_sound_volume: f32,
    pub preferred_3d_sound_volume: f32,
    pub preferred_speech_volume: f32,
    pub preferred_music_volume: f32,
    pub microphone_desired_height_above_terrain: f32,
    pub microphone_max_percentage_between_ground_and_camera: f32,
    pub zoom_min_distance: f32,
    pub zoom_max_distance: f32,
    pub zoom_sound_volume_percentage_amount: f32,
}

/// `parseSpeakerType`: an `AsciiString`, as its index in [`SPEAKER_TYPES`].
fn parse_speaker_type(ini: &mut Ini<'_>) -> IniResult<u32> {
    Ok(translate_speaker_type_to_unsigned_int(&ini.parse_ascii_string()?))
}

/// `audioSettingsFieldParseTable`.
pub static AUDIO_SETTINGS_FIELDS: [FieldParse<AudioSettings>; 37] = [
    FieldParse::new("AudioRoot", |i, s, _| i.parse_ascii_string().map(|v| s.audio_root = v), UserData::None),
    FieldParse::new("SoundsFolder", |i, s, _| i.parse_ascii_string().map(|v| s.sounds_folder = v), UserData::None),
    FieldParse::new("MusicFolder", |i, s, _| i.parse_ascii_string().map(|v| s.music_folder = v), UserData::None),
    FieldParse::new("StreamingFolder", |i, s, _| i.parse_ascii_string().map(|v| s.streaming_folder = v), UserData::None),
    FieldParse::new("SoundsExtension", |i, s, _| i.parse_ascii_string().map(|v| s.sounds_extension = v), UserData::None),
    FieldParse::new("UseDigital", |i, s, _| i.parse_bool().map(|v| s.use_digital = v), UserData::None),
    FieldParse::new("UseMidi", |i, s, _| i.parse_bool().map(|v| s.use_midi = v), UserData::None),
    FieldParse::new("OutputRate", |i, s, _| i.parse_int().map(|v| s.output_rate = v), UserData::None),
    FieldParse::new("OutputBits", |i, s, _| i.parse_int().map(|v| s.output_bits = v), UserData::None),
    FieldParse::new("OutputChannels", |i, s, _| i.parse_int().map(|v| s.output_channels = v), UserData::None),
    FieldParse::new("SampleCount2D", |i, s, _| i.parse_int().map(|v| s.sample_count_2d = v), UserData::None),
    FieldParse::new("SampleCount3D", |i, s, _| i.parse_int().map(|v| s.sample_count_3d = v), UserData::None),
    FieldParse::new("StreamCount", |i, s, _| i.parse_int().map(|v| s.stream_count = v), UserData::None),
    FieldParse::new("Preferred3DHW1", |i, s, _| i.parse_ascii_string().map(|v| s.preferred_3d_provider[0] = v), UserData::None),
    FieldParse::new("Preferred3DHW2", |i, s, _| i.parse_ascii_string().map(|v| s.preferred_3d_provider[1] = v), UserData::None),
    FieldParse::new("Preferred3DHW3", |i, s, _| i.parse_ascii_string().map(|v| s.preferred_3d_provider[2] = v), UserData::None),
    FieldParse::new("Preferred3DHW4", |i, s, _| i.parse_ascii_string().map(|v| s.preferred_3d_provider[3] = v), UserData::None),
    FieldParse::new("Preferred3DSW", |i, s, _| i.parse_ascii_string().map(|v| s.preferred_3d_provider[4] = v), UserData::None),
    FieldParse::new("Default2DSpeakerType", |i, s, _| parse_speaker_type(i).map(|v| s.default_speaker_type_2d = v), UserData::None),
    FieldParse::new("Default3DSpeakerType", |i, s, _| parse_speaker_type(i).map(|v| s.default_speaker_type_3d = v), UserData::None),
    FieldParse::new("MinSampleVolume", |i, s, _| i.parse_percent_to_real().map(|v| s.min_volume = v), UserData::None),
    FieldParse::new("GlobalMinRange", |i, s, _| i.parse_int().map(|v| s.global_min_range = v), UserData::None),
    FieldParse::new("GlobalMaxRange", |i, s, _| i.parse_int().map(|v| s.global_max_range = v), UserData::None),
    FieldParse::new("TimeBetweenDrawableSounds", |i, s, _| i.parse_duration_unsigned_int().map(|v| s.drawable_ambient_frames = v), UserData::None),
    FieldParse::new("TimeToFadeAudio", |i, s, _| i.parse_duration_unsigned_int().map(|v| s.fade_audio_frames = v), UserData::None),
    FieldParse::new("AudioFootprintInBytes", |i, s, _| i.parse_unsigned_int().map(|v| s.max_cache_size = v), UserData::None),
    FieldParse::new("Relative2DVolume", |i, s, _| i.parse_percent_to_real().map(|v| s.relative_2d_volume = v), UserData::None),
    FieldParse::new("DefaultSoundVolume", |i, s, _| i.parse_percent_to_real().map(|v| s.default_sound_volume = v), UserData::None),
    FieldParse::new("Default3DSoundVolume", |i, s, _| i.parse_percent_to_real().map(|v| s.default_3d_sound_volume = v), UserData::None),
    FieldParse::new("DefaultSpeechVolume", |i, s, _| i.parse_percent_to_real().map(|v| s.default_speech_volume = v), UserData::None),
    FieldParse::new("DefaultMusicVolume", |i, s, _| i.parse_percent_to_real().map(|v| s.default_music_volume = v), UserData::None),
    FieldParse::new("MicrophoneDesiredHeightAboveTerrain", |i, s, _| i.parse_real().map(|v| s.microphone_desired_height_above_terrain = v), UserData::None),
    FieldParse::new("MicrophoneMaxPercentageBetweenGroundAndCamera", |i, s, _| i.parse_percent_to_real().map(|v| s.microphone_max_percentage_between_ground_and_camera = v), UserData::None),
    FieldParse::new("ZoomMinDistance", |i, s, _| i.parse_real().map(|v| s.zoom_min_distance = v), UserData::None),
    FieldParse::new("ZoomMaxDistance", |i, s, _| i.parse_real().map(|v| s.zoom_max_distance = v), UserData::None),
    FieldParse::new("ZoomSoundVolumePercentageAmount", |i, s, _| i.parse_percent_to_real().map(|v| s.zoom_sound_volume_percentage_amount = v), UserData::None),
    FieldParse::end(),
];

/// The keys of the user's `Options.ini` that `OptionPreferences` reads here (`None`: no entry).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OptionPrefs {
    /// `3DAudioProvider`.
    pub audio_provider_3d: Option<Vec<u8>>,
    /// `SpeakerType`.
    pub speaker_type: Option<Vec<u8>>,
    /// `SFXVolume` (0..100).
    pub sfx_volume: Option<f32>,
    /// `SFX3DVolume`.
    pub sfx_3d_volume: Option<f32>,
    /// `VoiceVolume`.
    pub voice_volume: Option<f32>,
    /// `MusicVolume`.
    pub music_volume: Option<f32>,
}

impl OptionPrefs {
    /// `OptionPreferences::getSoundVolume`.
    pub fn get_sound_volume(&self, s: &AudioSettings) -> f32 {
        match self.sfx_volume {
            None => {
                let relative = s.relative_2d_volume;
                if relative < 0.0 {
                    let scale = 1.0f32 + relative;
                    return s.default_sound_volume * 100.0f32 * scale;
                }
                s.default_sound_volume * 100.0f32
            }
            Some(v) => if v < 0.0 { 0.0 } else { v },
        }
    }
    /// `OptionPreferences::get3DSoundVolume`.
    pub fn get_3d_sound_volume(&self, s: &AudioSettings) -> f32 {
        match self.sfx_3d_volume {
            None => {
                let relative = s.relative_2d_volume;
                if relative > 0.0 {
                    let scale = 1.0f32 - relative;
                    return s.default_3d_sound_volume * 100.0f32 * scale;
                }
                s.default_3d_sound_volume * 100.0f32
            }
            Some(v) => if v < 0.0 { 0.0 } else { v },
        }
    }
    /// `OptionPreferences::getSpeechVolume`.
    pub fn get_speech_volume(&self, s: &AudioSettings) -> f32 {
        self.voice_volume.map_or(s.default_speech_volume * 100.0f32, |v| if v < 0.0 { 0.0 } else { v })
    }
    /// `OptionPreferences::getMusicVolume`.
    pub fn get_music_volume(&self, s: &AudioSettings) -> f32 {
        self.music_volume.map_or(s.default_music_volume * 100.0f32, |v| if v < 0.0 { 0.0 } else { v })
    }
    /// `OptionPreferences::getPreferred3DProvider`.
    pub fn get_preferred_3d_provider(&self, s: &AudioSettings) -> Vec<u8> {
        self.audio_provider_3d.clone().unwrap_or_else(|| s.preferred_3d_provider[MAX_HW_PROVIDERS].clone())
    }
    /// `OptionPreferences::getSpeakerType`.
    pub fn get_speaker_type(&self, s: &AudioSettings) -> Vec<u8> {
        self.speaker_type.clone().unwrap_or_else(|| translate_unsigned_int_to_speaker_type(s.default_speaker_type_2d).as_bytes().to_vec())
    }
}

/// `class MiscAudio`: the named sounds the game plays itself (each an `AudioEventRTS` upstream; its name here).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MiscAudio {
    pub radar_unit_under_attack_sound: AudioEventName,
    pub radar_harvester_under_attack_sound: AudioEventName,
    pub radar_structure_under_attack_sound: AudioEventName,
    pub radar_under_attack_sound: AudioEventName,
    pub radar_infiltration_sound: AudioEventName,
    pub radar_online_sound: AudioEventName,
    pub radar_offline_sound: AudioEventName,
    pub defector_timer_tick_sound: AudioEventName,
    pub defector_timer_ding_sound: AudioEventName,
    pub lockon_tick_sound: AudioEventName,
    pub all_cheer_sound: AudioEventName,
    pub battle_cry_sound: AudioEventName,
    pub gui_click_sound: AudioEventName,
    pub no_can_do_sound: AudioEventName,
    pub stealth_discovered_sound: AudioEventName,
    pub stealth_neutralized_sound: AudioEventName,
    pub money_deposit_sound: AudioEventName,
    pub money_withdraw_sound: AudioEventName,
    pub building_disabled: AudioEventName,
    pub building_reenabled: AudioEventName,
    pub vehicle_disabled: AudioEventName,
    pub vehicle_reenabled: AudioEventName,
    pub splatter_vehicle_pilots_brain: AudioEventName,
    pub terrorist_in_car_move_voice: AudioEventName,
    pub terrorist_in_car_attack_voice: AudioEventName,
    pub terrorist_in_car_select_voice: AudioEventName,
    pub crate_heal: AudioEventName,
    pub crate_shroud: AudioEventName,
    pub crate_salvage: AudioEventName,
    pub crate_free_unit: AudioEventName,
    pub crate_money: AudioEventName,
    pub unit_promoted: AudioEventName,
    pub repair_sparks: AudioEventName,
    pub sabotage_shut_down_building: AudioEventName,
    pub sabotage_reset_timer_building: AudioEventName,
    pub aircraft_wheel_screech: AudioEventName,
}

macro_rules! misc_fields {
    ($($tok:literal => $f:ident),* $(,)?) => {
        /// `MiscAudio::m_fieldParseTable` (each `INI::parseAudioEventRTS`).
        pub static MISC_AUDIO_FIELDS: &[FieldParse<MiscAudio>] = &[
            $(FieldParse::new($tok, |i, m, _| m.$f.parse(i), UserData::None),)*
            FieldParse::end(),
        ];
    };
}

misc_fields! {
    "RadarNotifyUnitUnderAttackSound" => radar_unit_under_attack_sound,
    "RadarNotifyHarvesterUnderAttackSound" => radar_harvester_under_attack_sound,
    "RadarNotifyStructureUnderAttackSound" => radar_structure_under_attack_sound,
    "RadarNotifyUnderAttackSound" => radar_under_attack_sound,
    "RadarNotifyInfiltrationSound" => radar_infiltration_sound,
    "RadarNotifyOnlineSound" => radar_online_sound,
    "RadarNotifyOfflineSound" => radar_offline_sound,
    "DefectorTimerTickSound" => defector_timer_tick_sound,
    "DefectorTimerDingSound" => defector_timer_ding_sound,
    "LockonTickSound" => lockon_tick_sound,
    "AllCheerSound" => all_cheer_sound,
    "BattleCrySound" => battle_cry_sound,
    "GUIClickSound" => gui_click_sound,
    "NoCanDoSound" => no_can_do_sound,
    "StealthDiscoveredSound" => stealth_discovered_sound,
    "StealthNeutralizedSound" => stealth_neutralized_sound,
    "MoneyDepositSound" => money_deposit_sound,
    "MoneyWithdrawSound" => money_withdraw_sound,
    "BuildingDisabled" => building_disabled,
    "BuildingReenabled" => building_reenabled,
    "VehicleDisabled" => vehicle_disabled,
    "VehicleReenabled" => vehicle_reenabled,
    "SplatterVehiclePilotsBrain" => splatter_vehicle_pilots_brain,
    "TerroristInCarMoveVoice" => terrorist_in_car_move_voice,
    "TerroristInCarAttackVoice" => terrorist_in_car_attack_voice,
    "TerroristInCarSelectVoice" => terrorist_in_car_select_voice,
    "CrateHeal" => crate_heal,
    "CrateShroud" => crate_shroud,
    "CrateSalvage" => crate_salvage,
    "CrateFreeUnit" => crate_free_unit,
    "CrateMoney" => crate_money,
    "UnitPromoted" => unit_promoted,
    "RepairSparks" => repair_sparks,
    "SabotageShutDownBuilding" => sabotage_shut_down_building,
    "SabotageResetTimeBuilding" => sabotage_reset_timer_building,
    "AircraftWheelScreech" => aircraft_wheel_screech,
}

/// The audio's static data, as `AudioManager` holds it: the descriptions (`m_allAudioEventInfo`), the music track list
/// (`m_musicTracks`), the settings and the miscellaneous sounds.
#[derive(Clone, Debug, Default)]
pub struct AudioTables {
    /// `m_allAudioEventInfo`, by `m_audioName` (exact case, as upstream's hash map).
    pub infos: BTreeMap<Vec<u8>, Arc<AudioEventInfo>>,
    /// `m_musicTracks`, in the order added.
    pub music_tracks: Vec<Vec<u8>>,
    /// `m_audioSettings`.
    pub settings: AudioSettings,
    /// `m_miscAudio`.
    pub misc: MiscAudio,
    /// The user's preferences that `parseAudioSettingsDefinition` reads.
    pub prefs: OptionPrefs,
    /// `setPreferredProvider(prefs.getPreferred3DProvider())` (the device's; kept).
    pub preferred_provider: Vec<u8>,
    /// `setPreferredSpeaker(prefs.getSpeakerType())` (the device's; kept).
    pub preferred_speaker: Vec<u8>,
    /// `newAudioEventInfo` of a name already defined (upstream's `DEBUG_CRASH`, ignored in release): the names, in order.
    pub redefined: Vec<Vec<u8>>,
}

/// `AudioManager::init`'s files, in its order.
pub const AUDIO_INI_FILES: &[&[u8]] = &[
    b"Data\\INI\\AudioSettings.ini",
    b"Data\\INI\\Default\\Music.ini",
    b"Data\\INI\\Music.ini",
    b"Data\\INI\\Default\\SoundEffects.ini",
    b"Data\\INI\\SoundEffects.ini",
    b"Data\\INI\\Default\\Speech.ini",
    b"Data\\INI\\Speech.ini",
    b"Data\\INI\\Default\\Voice.ini",
    b"Data\\INI\\Voice.ini",
    b"Data\\INI\\MiscAudio.ini",
];

/// The block types of the audio's INI files (each a row of `theTypeTable`).
pub static AUDIO_BLOCKS: &[BlockParse<AudioTables>] = &[
    BlockParse { token: "AudioEvent", parse: parse_audio_event_definition },
    BlockParse { token: "AudioSettings", parse: parse_audio_settings_definition },
    BlockParse { token: "DialogEvent", parse: parse_dialog_definition },
    BlockParse { token: "MiscAudio", parse: parse_misc_audio },
    BlockParse { token: "MusicTrack", parse: parse_music_track_definition },
];

/// The common body of the three definitions: `newAudioEventInfo(name)`, the copy of the default (if it exists), the
/// name and type, then `initFromINI`. Returns whether the default existed.
fn parse_definition(ini: &mut Ini<'_>, t: &mut AudioTables, default: &[u8], ty: AudioType) -> IniResult<(Vec<u8>, bool)> {
    let name = ini.get_next_token(None)?;
    t.new_audio_event_info(&name);
    let default = t.infos.get(default).map(|d| (**d).clone());
    let had_default = default.is_some();
    let track = Arc::make_mut(t.infos.get_mut(&name).expect("made"));
    if let Some(d) = default {
        *track = d;
    }
    track.audio_name = name.clone();
    track.sound_type = ty;
    ini.init_from_ini(track, &AUDIO_EVENT_INFO_FIELDS)?;
    Ok((name, had_default))
}

/// `INI::parseMusicTrackDefinition`: as an audio event of type music, from `DefaultMusicTrack`; the track list gets the
/// name only when `DefaultMusicTrack` exists (added before the fields are read).
fn parse_music_track_definition(ini: &mut Ini<'_>, t: &mut AudioTables) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    t.new_audio_event_info(&name);
    let default = t.infos.get(b"DefaultMusicTrack".as_slice()).map(|d| (**d).clone());
    if default.is_some() {
        t.music_tracks.push(name.clone());
    }
    let track = Arc::make_mut(t.infos.get_mut(&name).expect("made"));
    if let Some(d) = default {
        *track = d;
    }
    track.audio_name = name;
    track.sound_type = AudioType::Music;
    ini.init_from_ini(track, &AUDIO_EVENT_INFO_FIELDS)
}

/// `INI::parseAudioEventDefinition`: a sound effect, from `DefaultSoundEffect`.
fn parse_audio_event_definition(ini: &mut Ini<'_>, t: &mut AudioTables) -> IniResult<()> {
    parse_definition(ini, t, b"DefaultSoundEffect", AudioType::SoundEffect).map(|_| ())
}

/// `INI::parseDialogDefinition`: a streamed speech, from `DefaultDialog`.
fn parse_dialog_definition(ini: &mut Ini<'_>, t: &mut AudioTables) -> IniResult<()> {
    parse_definition(ini, t, b"DefaultDialog", AudioType::Streaming).map(|_| ())
}

/// `INI::parseAudioSettingsDefinition`: the fields, then the preferences: the provider and speaker, the relative 2-D
/// volume clamped to [-1, 1] (and, as upstream, not stored back), and the four preferred volumes in hundredths.
fn parse_audio_settings_definition(ini: &mut Ini<'_>, t: &mut AudioTables) -> IniResult<()> {
    ini.init_from_ini(&mut t.settings, &AUDIO_SETTINGS_FIELDS)?;
    t.preferred_provider = t.prefs.get_preferred_3d_provider(&t.settings);
    t.preferred_speaker = t.prefs.get_speaker_type(&t.settings);
    // (upstream clamps the relative 2-D volume to [-1, 1] into a local here and never reads it)
    let p = &t.prefs;
    let s = &t.settings;
    let (a, b, c, d) = (p.get_sound_volume(s) / 100.0f32, p.get_3d_sound_volume(s) / 100.0f32, p.get_speech_volume(s) / 100.0f32, p.get_music_volume(s) / 100.0f32);
    t.settings.preferred_sound_volume = a;
    t.settings.preferred_3d_sound_volume = b;
    t.settings.preferred_speech_volume = c;
    t.settings.preferred_music_volume = d;
    Ok(())
}

/// `INI::parseMiscAudio`.
fn parse_misc_audio(ini: &mut Ini<'_>, t: &mut AudioTables) -> IniResult<()> {
    ini.init_from_ini(&mut t.misc, MISC_AUDIO_FIELDS)
}

impl AudioTables {
    pub fn new() -> Self {
        Self::default()
    }

    /// `INI::load(path, INI_LOAD_OVERWRITE)` of one audio INI file.
    pub fn load_text(&mut self, path: &[u8], text: &[u8]) -> IniResult<()> {
        Ini::load(path, text, IniLoadType::Overwrite, None, AUDIO_BLOCKS, self)
    }

    /// `AudioManager::init`'s loads ([`AUDIO_INI_FILES`]), each file that `open` has. Returns the files that did not parse.
    pub fn load(&mut self, open: impl Fn(&[u8]) -> Option<Vec<u8>>) -> Vec<(Vec<u8>, String)> {
        let mut errors = Vec::new();
        for p in AUDIO_INI_FILES {
            if let Some(t) = open(p) {
                if let Err(e) = self.load_text(p, &t) {
                    errors.push((p.to_vec(), format!("{e:?}")));
                }
            }
        }
        errors
    }

    /// `newAudioEventInfo(name)`: the existing description of that name (upstream's `DEBUG_CRASH`, recorded in
    /// [`AudioTables::redefined`]), else a new, zeroed one.
    pub fn new_audio_event_info(&mut self, name: &[u8]) -> &mut AudioEventInfo {
        if self.infos.contains_key(name) {
            self.redefined.push(name.to_vec());
        }
        Arc::make_mut(self.infos.entry(name.to_vec()).or_default())
    }

    /// `findAudioEventInfo(name)`.
    pub fn find_audio_event_info(&self, name: &[u8]) -> Option<&Arc<AudioEventInfo>> {
        self.infos.get(name)
    }

    /// `addTrackName(name)`.
    pub fn add_track_name(&mut self, name: &[u8]) {
        self.music_tracks.push(name.to_vec());
    }

    /// `nextTrackName(current)`: the track after `current` in the list, wrapping; the first if `current` is not in it;
    /// empty for no tracks.
    pub fn next_track_name(&self, current: &[u8]) -> Vec<u8> {
        let t = &self.music_tracks;
        let i = t.iter().position(|n| n == current).map_or(t.len(), |i| i + 1);
        t.get(i).or_else(|| t.first()).cloned().unwrap_or_default()
    }

    /// `prevTrackName(current)`: the track before `current`, wrapping to the last; the last if `current` is not in it.
    pub fn prev_track_name(&self, current: &[u8]) -> Vec<u8> {
        let t = &self.music_tracks;
        // the reverse iterator: the position from the end
        let r = t.iter().rev().position(|n| n == current).map_or(t.len(), |r| r + 1);
        t.iter().rev().nth(r).or_else(|| t.last()).cloned().unwrap_or_default()
    }

    /// `findAllAudioEventsOfType(type)` (in name order here; upstream's hash order is unspecified).
    pub fn find_all_audio_events_of_type(&self, ty: AudioType) -> Vec<&Arc<AudioEventInfo>> {
        self.infos.values().filter(|i| i.sound_type == ty).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &[u8] = b"\
AudioSettings\n\
  AudioRoot = Data\\Audio\n  SoundsFolder = Sounds\n  MusicFolder = Tracks\n  StreamingFolder = Speech\n\
  SoundsExtension = wav\n  SampleCount2D = 6\n  SampleCount3D = 24\n  StreamCount = 3\n\
  MinSampleVolume = 3%\n  GlobalMinRange = 150\n  GlobalMaxRange = 400\n  TimeToFadeAudio = 1000\n\
  Relative2DVolume = -10%\n  DefaultSoundVolume = 80%\n  Default3DSoundVolume = 60%\n  DefaultSpeechVolume = 70%\n\
  DefaultMusicVolume = 50%\n  Default2DSpeakerType = Headphones\n  Preferred3DSW = \"Miles Fast 2D Positional Audio\"\n\
End\n\
AudioEvent DefaultSoundEffect\n  Volume = 80%\n  Priority = normal\n  Type = world shrouded everyone\n  MinRange = 50\n  MaxRange = 300\nEnd\n\
AudioEvent Boom\n  Sounds = boom1 boom2,boom3\n  PitchShift = -10 5\n  Delay = 20 40\n  Control = random interrupt\n  Limit = 2\nEnd\n\
AudioEvent Boom\n  Volume = 50%\nEnd\n\
DialogEvent EvaHi\n  Filename = evahi.wav\n  Type = ui player\nEnd\n\
MusicTrack Early\n  Filename = early.mp3\nEnd\n\
MusicTrack DefaultMusicTrack\n  Volume = 90%\nEnd\n\
MusicTrack Late\n  Filename = late.mp3\nEnd\n\
MiscAudio\n  MoneyDepositSound = Deposit\n  NoCanDoSound = NoSound\nEnd\n";

    /// Predicted: the default sound effect's fields reach `Boom` (80 %, NORMAL, WORLD|SHROUDED|EVERYONE = 0x106, ranges
    /// 50..300); its pitch shift is 0.9..1.05, its delay 20..40, RANDOM|INTERRUPT = 0x12; the second `Boom` block
    /// starts over from the default (so its sounds are gone and its volume is 50 %) and is recorded as redefined;
    /// `EvaHi` is streaming, its type UI|PLAYER = 0x21; `Early` comes before `DefaultMusicTrack` so it is not a track,
    /// `Late` is, and copies the default's 90 %; ~~only `Late` is listed~~ (**missed**, 2026-11-06: `DefaultMusicTrack`
    /// is listed too, before `Late`: `newAudioEventInfo` makes it, so its own lookup of the default finds itself, as
    /// upstream's does); the settings read with no preferences give the preferred sound volume
    /// 0.8 * 0.9 = 0.72 (relative -10 %), 3-D 0.6, speech 0.7, music 0.5; the speaker type is 1; `NoSound` leaves
    /// `NoCanDoSound` empty.
    #[test]
    fn the_audio_ini_blocks_parse_as_upstream() {
        let mut t = AudioTables::new();
        t.load_text(b"a.ini", TEXT).expect("parses");
        let boom = t.find_audio_event_info(b"Boom").unwrap();
        assert_eq!(boom.volume, 0.5);
        assert!(boom.sounds.is_empty());
        assert_eq!(boom.priority, AudioPriority::Normal);
        assert_eq!(boom.sound_type_bits, ST_WORLD | ST_SHROUDED | ST_EVERYONE);
        assert_eq!((boom.min_distance, boom.max_distance), (50.0, 300.0));
        assert_eq!(boom.sound_type, AudioType::SoundEffect);
        assert_eq!(t.redefined, vec![b"Boom".to_vec()]);
        let mut t2 = AudioTables::new();
        t2.load_text(b"a.ini", b"AudioEvent Boom\n  Sounds = boom1 boom2,boom3\n  PitchShift = -10 5\n  Delay = 20 40\n  Control = random interrupt\n  Limit = 2\nEnd\n").expect("parses");
        let b1 = t2.find_audio_event_info(b"Boom").unwrap();
        assert_eq!(b1.sounds, vec![b"boom1".to_vec(), b"boom2".to_vec(), b"boom3".to_vec()]);
        assert_eq!((b1.pitch_shift_min, b1.pitch_shift_max), (1.0 - 10.0 / 100.0, 1.0 + 5.0 / 100.0));
        assert_eq!((b1.delay_min, b1.delay_max, b1.control, b1.limit), (20, 40, AC_RANDOM | AC_INTERRUPT, 2));
        let eva = t.find_audio_event_info(b"EvaHi").unwrap();
        assert_eq!((eva.sound_type, eva.sound_type_bits), (AudioType::Streaming, ST_UI | ST_PLAYER));
        assert_eq!(t.music_tracks, vec![b"DefaultMusicTrack".to_vec(), b"Late".to_vec()]);
        assert_eq!(t.find_audio_event_info(b"Late").unwrap().volume, 0.9);
        assert_eq!(t.settings.preferred_sound_volume, 0.8f32 * 100.0 * (1.0f32 - 0.1) / 100.0);
        assert_eq!(t.settings.preferred_3d_sound_volume, 0.6f32 * 100.0 / 100.0);
        assert_eq!(t.settings.preferred_music_volume, 0.5);
        assert_eq!(t.settings.default_speaker_type_2d, 1);
        assert_eq!(t.preferred_speaker, b"Headphones");
        assert_eq!(t.preferred_provider, b"Miles Fast 2D Positional Audio");
        assert_eq!(t.settings.fade_audio_frames, 30);
        assert_eq!(t.misc.money_deposit_sound.event_name, b"Deposit");
        assert!(t.misc.no_can_do_sound.event_name.is_empty());
    }

    /// Predicted: next after the last wraps to the first, next of an unknown name is the first, previous of the first is
    /// the last, and no tracks give an empty name.
    #[test]
    fn the_track_list_wraps() {
        let mut t = AudioTables::new();
        assert!(t.next_track_name(b"x").is_empty());
        for n in [b"A", b"B", b"C"] {
            t.add_track_name(n);
        }
        assert_eq!(t.next_track_name(b"A"), b"B");
        assert_eq!(t.next_track_name(b"C"), b"A");
        assert_eq!(t.next_track_name(b"zz"), b"A");
        assert_eq!(t.prev_track_name(b"A"), b"C");
        assert_eq!(t.prev_track_name(b"C"), b"B");
        assert_eq!(t.prev_track_name(b"zz"), b"C");
        assert_eq!(translate_unsigned_int_to_speaker_type(9), "2 Speakers");
        assert_eq!(translate_speaker_type_to_unsigned_int(b"5.1 Surround"), 4);
        assert_eq!(translate_speaker_type_to_unsigned_int(b"5.1 surround"), 0);
    }
}
