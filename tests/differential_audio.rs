// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of the audio's static data and its events (GitHub #13; `client::audio_info` and
//! `client::audio_event`) against upstream's own `INIAudioEventInfo.cpp`, `INIMiscAudio.cpp`, GameAudio.cpp's settings
//! table and `parseAudioSettingsDefinition`, INI.cpp's `parseAudioEventRTS`, OptionsMenu.cpp's audio preference getters
//! and all of `AudioEventRTS.cpp` (commit 0a05454d), compiled by `build.rs` with the real `AudioEventInfo.h`,
//! `AudioEventRTS.h`, `AudioSettings.h`, `MiscAudio.h` and `INI.h` and driven by `differential/wrap_audio.cpp` (which
//! lists its stand-ins: the manager's table, the file system, the language, objects and drawables, the user's
//! preferences). The INI is read by the real `INI::load` (wrap_ini.cpp's `ref_ini_load`).
//!
//! No game files are used: every block is **synthetic**.
//!
//! # Methodology
//!
//! - **The INI** (`audio_ini_matches_upstream`, 4,000 cases): one or two random files per case of one to eight blocks
//!   (`AudioEvent`, `DialogEvent`, `MusicTrack`, `AudioSettings`, `MiscAudio`), names drawn from a small pool that
//!   includes the three defaults (so blocks copy defaults, redefine names, and a default copies itself), each block a
//!   random subset of its fields in a random order, with valid values and damaged ones (a word for a number, an unknown
//!   priority or type name, `+`/`-` and `NONE` type lists, a missing token, an unknown field); sometimes the user's
//!   preferences set. Compared: the outcome (success, or the thrown status and message) and every field of every
//!   description, the track list, every setting, every misc sound's name and the preferred provider and speaker.
//! - **The events** (`audio_events_match_upstream`, 4,000 cases): a valid table, objects with players, files that
//!   exist (some localized, under one of two languages), then random events: a name (defined or not), an owner (none,
//!   an object, a drawable, a position; some owners missing), logical or not, a player index, and a run of
//!   `generateFilename` and `generatePlayInfo` calls; both sides' audio and logic generators start from the same state.
//!   **GitHub #50:** a logical event is run as `addAudioEvent` runs it, `generateFilename` then `generatePlayInfo` once
//!   each: upstream's draws its picks from its compiled logic stream; the Rust's logic makes its draws first
//!   (`logic::logical_audio::logical_audio_draws`, from the Rust logic stream) and the client's event plays those values
//!   (`AudioEventRts::logic_picks`). The final words of both logic streams are compared as before.
//!   Compared after each call: the file, the playing index, the delay, the pitch and volume shifts, the loop count, the
//!   attack and decay files, the next portion and the logical flag; then `isPositionalAudio`, `getVolume`,
//!   `getCurrentPosition`, the owner type and `getPlayerIndex`; and the generators' words at the end.
//!
//! # Results
//!
//! See the `DIFF` lines of a run; recorded in `docs/phase2-client.md` (#13).
#![cfg(feature = "differential")]

mod support;

use std::ffi::CString;
use std::os::raw::{c_char, c_int};

use rts_lockstep_engine::client::audio_event::{AudioEventHost, AudioEventRts, OwnerType, PortionToPlay};
use rts_lockstep_engine::client::audio_info::{AudioEventInfo, AudioSettings, AudioTables, MiscAudio, OptionPrefs, MAX_HW_PROVIDERS};
use rts_lockstep_engine::common::audio_event_name::AudioEventName;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::ini::{IniResult, Thrown};
use rts_lockstep_engine::common::random_value::{seed_random, RandomValues};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::logical_audio::logical_audio_draws;
use support::Gen;

struct Words {
    name: &'static str,
    compared: u64,
    mismatches: u64,
    examples: Vec<String>,
}

impl Words {
    fn new(name: &'static str) -> Self {
        Words { name, compared: 0, mismatches: 0, examples: Vec::new() }
    }
    fn cmp(&mut self, rust: &[u32], cpp: &[u32], ctx: impl Fn() -> String) {
        self.compared += 1;
        if let Some(i) = (0..rust.len().max(cpp.len())).find(|&i| rust.get(i) != cpp.get(i)) {
            self.mismatches += 1;
            if self.examples.len() < 4 {
                let lo = i.saturating_sub(4);
                let show = |v: &[u32]| v[lo.min(v.len())..(i + 6).min(v.len())].iter().map(|w| format!("{w:#x}")).collect::<Vec<_>>().join(" ");
                self.examples.push(format!("word {i} of {}/{}: rust [{}] c++ [{}]\n{}", rust.len(), cpp.len(), show(rust), show(cpp), ctx()));
            }
        }
    }
    fn finish(self) {
        println!("DIFF {:<36} compared {:>8}  mismatches {:>5}", self.name, self.compared, self.mismatches);
        assert!(self.compared > 0, "{}: nothing compared", self.name);
        assert_eq!(self.mismatches, 0, "{}: {} of {} differ; first: {:#?}", self.name, self.mismatches, self.compared, self.examples);
    }
}

fn cstr(s: &str) -> CString {
    CString::new(s).expect("no NUL")
}

// --------------------------------------------------------------------------- the dumps (wrap_audio.cpp's)

fn wstr(v: &mut Vec<u32>, s: &[u8]) {
    v.push(s.len() as u32);
    v.extend(s.iter().map(|&b| u32::from(b)));
}
fn wvec(v: &mut Vec<u32>, l: &[Vec<u8>]) {
    v.push(l.len() as u32);
    for s in l {
        wstr(v, s);
    }
}
fn wf(v: &mut Vec<u32>, x: f32) {
    v.push(x.to_bits());
}

fn dump_info(v: &mut Vec<u32>, i: &AudioEventInfo) {
    wstr(v, &i.audio_name);
    wstr(v, &i.filename);
    for x in [i.volume, i.volume_shift, i.min_volume, i.pitch_shift_min, i.pitch_shift_max] {
        wf(v, x);
    }
    v.extend([i.delay_min as u32, i.delay_max as u32, i.limit as u32, i.loop_count as u32, i.priority as u32, i.sound_type_bits, i.control]);
    for l in [&i.sounds_morning, &i.sounds, &i.sounds_night, &i.sounds_evening, &i.attack_sounds, &i.decay_sounds] {
        wvec(v, l);
    }
    for x in [i.low_pass_freq, i.min_distance, i.max_distance] {
        wf(v, x);
    }
    v.push(i.sound_type as u32);
}

fn dump_settings(v: &mut Vec<u32>, s: &AudioSettings) {
    for x in [&s.audio_root, &s.sounds_folder, &s.music_folder, &s.streaming_folder, &s.sounds_extension] {
        wstr(v, x);
    }
    v.extend([u32::from(s.use_digital), u32::from(s.use_midi)]);
    for x in [s.output_rate, s.output_bits, s.output_channels, s.sample_count_2d, s.sample_count_3d, s.stream_count, s.global_min_range, s.global_max_range] {
        v.push(x as u32);
    }
    v.extend([s.drawable_ambient_frames, s.fade_audio_frames, s.max_cache_size]);
    wf(v, s.min_volume);
    for i in 0..=MAX_HW_PROVIDERS {
        wstr(v, &s.preferred_3d_provider[i]);
    }
    for x in [s.relative_2d_volume, s.default_sound_volume, s.default_3d_sound_volume, s.default_speech_volume, s.default_music_volume] {
        wf(v, x);
    }
    v.extend([s.default_speaker_type_2d, s.default_speaker_type_3d]);
    for x in [
        s.preferred_sound_volume,
        s.preferred_3d_sound_volume,
        s.preferred_speech_volume,
        s.preferred_music_volume,
        s.microphone_desired_height_above_terrain,
        s.microphone_max_percentage_between_ground_and_camera,
        s.zoom_min_distance,
        s.zoom_max_distance,
        s.zoom_sound_volume_percentage_amount,
    ] {
        wf(v, x);
    }
}

/// `MiscAudio::m_fieldParseTable`'s order.
fn misc_names(m: &MiscAudio) -> [&AudioEventName; 36] {
    [
        &m.radar_unit_under_attack_sound,
        &m.radar_harvester_under_attack_sound,
        &m.radar_structure_under_attack_sound,
        &m.radar_under_attack_sound,
        &m.radar_infiltration_sound,
        &m.radar_online_sound,
        &m.radar_offline_sound,
        &m.defector_timer_tick_sound,
        &m.defector_timer_ding_sound,
        &m.lockon_tick_sound,
        &m.all_cheer_sound,
        &m.battle_cry_sound,
        &m.gui_click_sound,
        &m.no_can_do_sound,
        &m.stealth_discovered_sound,
        &m.stealth_neutralized_sound,
        &m.money_deposit_sound,
        &m.money_withdraw_sound,
        &m.building_disabled,
        &m.building_reenabled,
        &m.vehicle_disabled,
        &m.vehicle_reenabled,
        &m.splatter_vehicle_pilots_brain,
        &m.terrorist_in_car_move_voice,
        &m.terrorist_in_car_attack_voice,
        &m.terrorist_in_car_select_voice,
        &m.crate_heal,
        &m.crate_shroud,
        &m.crate_salvage,
        &m.crate_free_unit,
        &m.crate_money,
        &m.unit_promoted,
        &m.repair_sparks,
        &m.sabotage_shut_down_building,
        &m.sabotage_reset_timer_building,
        &m.aircraft_wheel_screech,
    ]
}

const MISC_TOKENS: [&str; 36] = [
    "RadarNotifyUnitUnderAttackSound",
    "RadarNotifyHarvesterUnderAttackSound",
    "RadarNotifyStructureUnderAttackSound",
    "RadarNotifyUnderAttackSound",
    "RadarNotifyInfiltrationSound",
    "RadarNotifyOnlineSound",
    "RadarNotifyOfflineSound",
    "DefectorTimerTickSound",
    "DefectorTimerDingSound",
    "LockonTickSound",
    "AllCheerSound",
    "BattleCrySound",
    "GUIClickSound",
    "NoCanDoSound",
    "StealthDiscoveredSound",
    "StealthNeutralizedSound",
    "MoneyDepositSound",
    "MoneyWithdrawSound",
    "BuildingDisabled",
    "BuildingReenabled",
    "VehicleDisabled",
    "VehicleReenabled",
    "SplatterVehiclePilotsBrain",
    "TerroristInCarMoveVoice",
    "TerroristInCarAttackVoice",
    "TerroristInCarSelectVoice",
    "CrateHeal",
    "CrateShroud",
    "CrateSalvage",
    "CrateFreeUnit",
    "CrateMoney",
    "UnitPromoted",
    "RepairSparks",
    "SabotageShutDownBuilding",
    "SabotageResetTimeBuilding",
    "AircraftWheelScreech",
];

fn rust_dump(t: &AudioTables) -> Vec<u32> {
    let mut v = vec![t.infos.len() as u32];
    for i in t.infos.values() {
        dump_info(&mut v, i);
    }
    wvec(&mut v, &t.music_tracks);
    dump_settings(&mut v, &t.settings);
    for n in misc_names(&t.misc) {
        wstr(&mut v, &n.event_name);
    }
    wstr(&mut v, &t.preferred_provider);
    wstr(&mut v, &t.preferred_speaker);
    v
}

fn cpp_dump() -> Vec<u32> {
    let mut out = vec![0u32; 1 << 18];
    let n = unsafe { cpp::ref_audio_dump(out.as_mut_ptr(), out.len() as c_int) } as usize;
    assert!(n <= out.len());
    out.truncate(n);
    out
}

/// (status, detail) of a load, as the particle test's: 0 ok, 1 an exception message, 2/3 a status or error code.
fn rust_outcome(r: IniResult<()>) -> Option<Vec<u32>> {
    let (status, detail) = match r {
        Ok(()) => (0, Vec::new()),
        Err(e) => match e.thrown() {
            None => return None,
            Some(Thrown::Exception(m)) => (1, m),
            Some(Thrown::Status(c)) => (2, c.to_le_bytes().to_vec()),
            Some(Thrown::ErrorCode(c)) => (3, c.to_le_bytes().to_vec()),
            Some(Thrown::OutOfRange) => (4, Vec::new()),
            Some(Thrown::Other) => (5, Vec::new()),
        },
    };
    Some(std::iter::once(status).chain(detail.into_iter().map(u32::from)).collect())
}

fn cpp_load(name: &str, data: &[u8]) -> Vec<u32> {
    let cname = cstr(name);
    let mut log = vec![0u8; 16];
    let mut msg = vec![0u8; 4096];
    let (mut log_len, mut crc, mut code) = (0i32, 0u32, 0u32);
    let status = unsafe {
        cpp::ref_ini_reset();
        cpp::ref_ini_add_file(cname.as_ptr(), data.as_ptr() as *const c_char, data.len() as c_int);
        cpp::ref_ini_load(cname.as_ptr(), 0, log.as_mut_ptr(), log.len() as c_int, &mut log_len, &mut crc, msg.as_mut_ptr() as *mut c_char, msg.len() as c_int, &mut code)
    };
    assert_eq!(log_len, 0, "the real block handlers log nothing");
    let detail = match status {
        0 | 4 => Vec::new(),
        1 => msg[..code as usize].to_vec(),
        2 | 3 => code.to_le_bytes().to_vec(),
        _ => panic!("C++ threw something unexpected (status {status})"),
    };
    std::iter::once(status as u32).chain(detail.into_iter().map(u32::from)).collect()
}

// --------------------------------------------------------------------------- the INI generator

const NAMES: [&str; 8] = ["DefaultSoundEffect", "DefaultDialog", "DefaultMusicTrack", "Alpha", "Beta", "Gamma", "Game_Track", "Delta"];
const PRIORITIES: [&str; 7] = ["LOWEST", "low", "Normal", "HIGH", "critical", "URGENT", "2"];
const TYPES: [&str; 10] = ["UI", "world", "SHROUDED", "GLOBAL", "voice", "PLAYER", "ALLIES", "ENEMIES", "EVERYONE", "SOMETIMES"];
const CONTROLS: [&str; 6] = ["LOOP", "random", "ALL", "POSTDELAY", "INTERRUPT", "FOREVER"];
const SPEAKERS: [&str; 7] = ["2 Speakers", "Headphones", "Surround Sound", "4 Speaker", "5.1 Surround", "7.1 Surround", "Mono"];

fn pick<'a>(g: &mut Gen, l: &[&'a str]) -> &'a str {
    l[g.below(l.len() as u32) as usize]
}

/// A number token: mostly an integer or a decimal, sometimes a word.
fn num(g: &mut Gen, lo: i32, hi: i32) -> String {
    match g.below(12) {
        0 => "loud".into(),
        1 => format!("{}.{}", g.below((hi - lo) as u32 + 1) as i32 + lo, g.below(100)),
        _ => format!("{}", g.below((hi - lo) as u32 + 1) as i32 + lo),
    }
}

fn pct(g: &mut Gen) -> String {
    format!("{}%", num(g, -100, 120))
}

fn bits(g: &mut Gen, names: &[&str]) -> String {
    match g.below(8) {
        0 => "NONE".into(),
        1 => (0..1 + g.below(3)).map(|_| format!("{}{}", if g.below(2) == 0 { "+" } else { "-" }, pick(g, names))).collect::<Vec<_>>().join(" "),
        2 => String::new(),
        _ => (0..1 + g.below(4)).map(|_| pick(g, names).to_string()).collect::<Vec<_>>().join(" "),
    }
}

fn sounds(g: &mut Gen) -> String {
    let words = ["boom", "Bang01", "x_y", "zap.wav", "s1", "s2"];
    (0..g.below(5)).map(|_| pick(g, &words).to_string()).collect::<Vec<_>>().join(if g.below(2) == 0 { " " } else { ", " })
}

fn event_field(g: &mut Gen) -> String {
    match g.below(21) {
        0 => format!("Filename = {}", pick(g, &["a.wav", "Track01.mp3", "speech\\eva.wav"])),
        1 => format!("Volume = {}", pct(g)),
        2 => format!("VolumeShift = {}", pct(g)),
        3 => format!("MinVolume = {}", pct(g)),
        4 => format!("PitchShift = {} {}", num(g, -90, 50), num(g, -50, 90)),
        5 => format!("Delay = {} {}", num(g, 0, 200), num(g, 0, 400)),
        6 => format!("Limit = {}", num(g, 0, 5)),
        7 => format!("LoopCount = {}", num(g, 0, 4)),
        8 => format!("Priority = {}", pick(g, &PRIORITIES)),
        9 => format!("Type = {}", bits(g, &TYPES)),
        10 => format!("Control = {}", bits(g, &CONTROLS)),
        11 => format!("Sounds = {}", sounds(g)),
        12 => format!("SoundsNight = {}", sounds(g)),
        13 => format!("SoundsEvening = {}", sounds(g)),
        14 => format!("SoundsMorning = {}", sounds(g)),
        15 => format!("Attack = {}", sounds(g)),
        16 => format!("Decay = {}", sounds(g)),
        17 => format!("MinRange = {}", num(g, 0, 400)),
        18 => format!("MaxRange = {}", num(g, 0, 1500)),
        19 => format!("LowPassCutoff = {}", pct(g)),
        _ => if g.below(3) == 0 { "Volume =".into() } else { "Loudness = 3".into() },
    }
}

fn settings_field(g: &mut Gen) -> String {
    let strs = ["AudioRoot", "SoundsFolder", "MusicFolder", "StreamingFolder", "SoundsExtension", "Preferred3DHW1", "Preferred3DHW2", "Preferred3DHW3", "Preferred3DHW4", "Preferred3DSW"];
    let ints = ["OutputRate", "OutputBits", "OutputChannels", "SampleCount2D", "SampleCount3D", "StreamCount", "GlobalMinRange", "GlobalMaxRange", "AudioFootprintInBytes"];
    let pcts = ["MinSampleVolume", "Relative2DVolume", "DefaultSoundVolume", "Default3DSoundVolume", "DefaultSpeechVolume", "DefaultMusicVolume", "MicrophoneMaxPercentageBetweenGroundAndCamera", "ZoomSoundVolumePercentageAmount"];
    let reals = ["MicrophoneDesiredHeightAboveTerrain", "ZoomMinDistance", "ZoomMaxDistance"];
    match g.below(8) {
        0 => {
            let f = pick(g, &strs);
            let v = pick(g, &["Data\\Audio", "Sounds", "wav", "\"Miles Fast 2D Positional Audio\"", "\"Dolby Surround\"", "mp3"]);
            format!("{f} = {v}")
        }
        1 | 2 => format!("{} = {}", pick(g, &ints), num(g, -5, 50000)),
        3 => format!("{} = {}", pick(g, &pcts), pct(g)),
        4 => format!("{} = {}", pick(g, &reals), num(g, 0, 500)),
        5 => format!("{} = {}", pick(g, &["UseDigital", "UseMidi"]), pick(g, &["Yes", "no", "maybe"])),
        6 => format!("{} = \"{}\"", pick(g, &["Default2DSpeakerType", "Default3DSpeakerType"]), pick(g, &SPEAKERS)),
        _ => format!("{} = {}", pick(g, &["TimeBetweenDrawableSounds", "TimeToFadeAudio"]), num(g, 0, 5000)),
    }
}

fn misc_field(g: &mut Gen) -> String {
    let v = pick(g, &["Alpha", "NoSound", "nosound", "Beta", "Unknown"]);
    if g.below(15) == 0 {
        return format!("{} =", pick(g, &MISC_TOKENS));
    }
    format!("{} = {v}", pick(g, &MISC_TOKENS))
}

fn gen_ini_file(g: &mut Gen) -> String {
    let mut s = String::new();
    for _ in 0..1 + g.below(8) {
        let kind = g.below(10);
        let (head, field): (String, fn(&mut Gen) -> String) = match kind {
            0..=3 => (format!("AudioEvent {}", pick(g, &NAMES)), event_field),
            4 | 5 => (format!("DialogEvent {}", pick(g, &NAMES)), event_field),
            6 | 7 => (format!("MusicTrack {}", pick(g, &NAMES)), event_field),
            8 => ("AudioSettings".into(), settings_field),
            _ => ("MiscAudio".into(), misc_field),
        };
        s.push_str(&head);
        s.push('\n');
        for _ in 0..g.below(9) {
            s.push_str("  ");
            s.push_str(&field(g));
            s.push('\n');
        }
        if g.below(40) != 0 {
            s.push_str("End\n");
        }
    }
    s
}

fn gen_prefs(g: &mut Gen) -> Vec<(&'static str, String)> {
    let mut p = Vec::new();
    if g.below(3) == 0 {
        for k in ["SFXVolume", "SFX3DVolume", "VoiceVolume", "MusicVolume", "3DAudioProvider", "SpeakerType"] {
            if g.below(2) == 0 {
                let v = if k.ends_with("Volume") { format!("{}", g.below(140) as i32 - 20) } else { pick(g, &["Headphones", "Miles 2D", "4 Speaker"]).to_string() };
                p.push((k, v));
            }
        }
    }
    p
}

fn apply_prefs(t: &mut AudioTables, prefs: &[(&str, String)]) {
    let mut o = OptionPrefs::default();
    for (k, v) in prefs {
        let f = || Some(v.parse::<f32>().expect("an integer"));
        match *k {
            "SFXVolume" => o.sfx_volume = f(),
            "SFX3DVolume" => o.sfx_3d_volume = f(),
            "VoiceVolume" => o.voice_volume = f(),
            "MusicVolume" => o.music_volume = f(),
            "3DAudioProvider" => o.audio_provider_3d = Some(v.as_bytes().to_vec()),
            _ => o.speaker_type = Some(v.as_bytes().to_vec()),
        }
        let (ck, cv) = (cstr(k), cstr(v));
        unsafe { cpp::ref_audio_pref(ck.as_ptr(), cv.as_ptr()) };
    }
    t.prefs = o;
}

#[test]
fn audio_ini_matches_upstream() {
    let _lock = cpp::GLOBAL_RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: u32 = std::env::var("AUDIO_INI_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(4000);
    let mut g = Gen::new(0x1300_0001);
    let mut outcomes = Words::new("audio INI outcome");
    let mut dumps = Words::new("audio INI tables");
    let (mut ok, mut failed, mut undefined, mut infos) = (0u64, 0u64, 0u64, 0u64);
    for case in 0..cases {
        unsafe { cpp::ref_audio_reset() };
        let mut t = AudioTables::new();
        let prefs = gen_prefs(&mut g);
        apply_prefs(&mut t, &prefs);
        for round in 0..1 + g.below(2) {
            let text = gen_ini_file(&mut g);
            let Some(r) = rust_outcome(t.load_text(b"a.ini", text.as_bytes())) else {
                undefined += 1;
                break;
            };
            let c = cpp_load("a.ini", text.as_bytes());
            if r[0] == 0 {
                ok += 1;
            } else {
                failed += 1;
            }
            let ctx = || format!("case {case} round {round} prefs {prefs:?}, text:\n{text}");
            outcomes.cmp(&r, &c, ctx);
            dumps.cmp(&rust_dump(&t), &cpp_dump(), ctx);
        }
        infos += t.infos.len() as u64;
    }
    println!("COVERAGE audio INI: {cases} cases, {ok} loads succeeded, {failed} failed, {undefined} undefined, {infos} descriptions compared at the end");
    outcomes.finish();
    dumps.finish();
    assert!(ok > u64::from(cases) / 4 && failed > 0, "both outcomes");
}

// --------------------------------------------------------------------------- the events

#[derive(Default)]
struct Host {
    files: Vec<Vec<u8>>,
    language: Vec<u8>,
    objects: Vec<(u32, Coord3D, i32)>,
}

impl AudioEventHost for Host {
    fn does_file_exist(&self, p: &[u8]) -> bool {
        self.files.iter().any(|f| f == p)
    }
    fn registry_language(&self) -> Vec<u8> {
        self.language.clone()
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

const EVENT_TABLE: &str = "AudioSettings\n AudioRoot = Data\\Audio\n SoundsFolder = Sounds\n MusicFolder = Tracks\n StreamingFolder = Speech\n SoundsExtension = wav\nEnd\n\
AudioEvent DefaultSoundEffect\n Volume = 70%\n Type = world everyone\nEnd\n\
AudioEvent Seq\n Sounds = a b c\n Delay = 0 50\nEnd\n\
AudioEvent Rnd\n Sounds = a b c d e\n Control = random loop\n LoopCount = 2\n PitchShift = -15 10\n VolumeShift = -40%\n Delay = 10 300\nEnd\n\
AudioEvent Two\n Sounds = a b\n Control = random\n Attack = at1 at2 at3\n Decay = de1 de2\n Type = ui player\nEnd\n\
AudioEvent One\n Sounds = solo\n Control = random all\n Decay = tail\nEnd\n\
AudioEvent None\n Type = world\nEnd\n\
DialogEvent Talk\n Filename = talk.wav\n Volume = 90%\nEnd\n\
MusicTrack DefaultMusicTrack\n Volume = 50%\nEnd\n\
MusicTrack Song\n Filename = song.mp3\nEnd\n";

#[test]
fn audio_events_match_upstream() {
    let _lock = cpp::GLOBAL_RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: u32 = std::env::var("AUDIO_EVENT_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(4000);
    let mut g = Gen::new(0x1300_0002);
    let mut states = Words::new("audio events (state per call)");
    let mut rng = Words::new("audio events (generators)");
    let mut localized = 0u64;
    let (mut logical_events, mut logic_draws) = (0u64, 0u64);
    for case in 0..cases {
        unsafe { cpp::ref_audio_reset() };
        let mut t = AudioTables::new();
        t.load_text(b"t.ini", EVENT_TABLE.as_bytes()).expect("the table");
        assert_eq!(cpp_load("t.ini", EVENT_TABLE.as_bytes())[0], 0);
        let mut h = Host { language: if g.below(2) == 0 { b"English".to_vec() } else { b"German".to_vec() }, ..Default::default() };
        let lang = cstr(std::str::from_utf8(&h.language).unwrap());
        unsafe { cpp::ref_audio_language(lang.as_ptr()) };
        for f in ["a", "c", "at2", "de1", "tail", "solo"] {
            if g.below(3) == 0 {
                let l = if g.below(2) == 0 { "English" } else { "German" };
                let p = format!("Data\\Audio\\Sounds\\{l}\\\\{f}.wav");
                h.files.push(p.clone().into_bytes());
                let c = cstr(&p);
                unsafe { cpp::ref_audio_file(c.as_ptr()) };
            }
        }
        if g.below(3) == 0 {
            let p = "Data\\Audio\\Speech\\English\\\\talk.wav";
            h.files.push(p.as_bytes().to_vec());
            unsafe { cpp::ref_audio_file(cstr(p).as_ptr()) };
        }
        for id in 1..4u32 {
            if g.below(4) != 0 {
                let p = Coord3D { x: g.uniform(-500.0, 500.0), y: g.uniform(-500.0, 500.0), z: g.uniform(0.0, 50.0) };
                let player = g.below(4) as i32;
                h.objects.push((id, p, player));
                unsafe { cpp::ref_audio_object(id, p.x, p.y, p.z, player) };
            }
        }
        let mut r = RandomValues::default();
        seed_random(g.u32(), &mut r.audio);
        seed_random(g.u32(), &mut r.logic);
        unsafe {
            cpp::ref_set_state(2, r.audio.as_ptr());
            cpp::ref_set_state(0, r.logic.as_ptr());
        }
        for ev in 0..1 + g.below(6) {
            let name = pick(&mut g, &["Seq", "Rnd", "Two", "One", "None", "Talk", "Song", "Missing", "DefaultSoundEffect"]);
            let owner = g.below(4) as i32;
            let id = g.below(5);
            let pos = [g.uniform(-300.0, 300.0), g.uniform(-300.0, 300.0), g.uniform(0.0, 30.0)];
            let logical = g.below(4) == 0;
            let player_index = g.below(5) as i32 - 1;
            let mut n_ops = 1 + g.below(8) as i32;
            let mut ops = g.u32() & ((1u32 << (2 * n_ops)) - 1);
            if logical {
                // (#50) `addAudioEvent`'s calls: generateFilename, then generatePlayInfo
                (n_ops, ops) = (1, 3);
                logical_events += 1;
            }
            // Rust
            let p = Coord3D { x: pos[0], y: pos[1], z: pos[2] };
            let mut e = match owner {
                1 => AudioEventRts::with_object(name.as_bytes(), id),
                2 => AudioEventRts::with_drawable(name.as_bytes(), id),
                3 => AudioEventRts::at_position(name.as_bytes(), &p),
                _ => AudioEventRts::new(name.as_bytes()),
            };
            e.is_logical_audio = logical;
            e.player_index = player_index;
            e.event_info = t.find_audio_event_info(name.as_bytes()).cloned();
            if logical {
                // (#50) the logic's draws, from the Rust logic stream, which the client's event then plays
                e.logic_picks = logical_audio_draws(&t, name.as_bytes(), &mut r);
                logic_draws += e.logic_picks.count() as u64;
            }
            let mut a = vec![u32::from(e.event_info.is_some())];
            if e.event_info.is_some() {
                for k in 0..n_ops {
                    let op = (ops >> (2 * k)) & 3;
                    if op & 1 != 0 {
                        e.generate_filename(&t.settings, &h, &mut r);
                    }
                    if op & 2 != 0 {
                        e.generate_play_info(&t.settings, &h, &mut r);
                    }
                    localized += u64::from(e.filename_to_load.windows(2).any(|w| w == b"\\\\"));
                    wstr(&mut a, &e.filename_to_load);
                    a.push(e.playing_audio_index as u32);
                    wf(&mut a, e.delay);
                    wf(&mut a, e.pitch_shift);
                    wf(&mut a, e.volume_shift);
                    a.push(e.loop_count as u32);
                    wstr(&mut a, &e.attack_name);
                    wstr(&mut a, &e.decay_name);
                    a.push(match e.portion_to_play_next {
                        PortionToPlay::Attack => 0,
                        PortionToPlay::Sound => 1,
                        PortionToPlay::Decay => 2,
                        PortionToPlay::Done => 3,
                    });
                    a.push(u32::from(e.is_logical_audio));
                }
            }
            a.push(u32::from(e.is_positional_audio()));
            wf(&mut a, e.get_volume());
            match e.get_current_position(&h) {
                Some(c) => {
                    a.push(1);
                    wf(&mut a, c.x);
                    wf(&mut a, c.y);
                    wf(&mut a, c.z);
                }
                None => a.extend([0, 0, 0, 0]),
            }
            a.push(match e.owner_type {
                OwnerType::Positional => 0,
                OwnerType::Drawable => 1,
                OwnerType::Object => 2,
                OwnerType::Dead => 3,
                OwnerType::Invalid => 4,
            });
            a.push(e.get_player_index(&h) as u32);
            // C++
            let cname = cstr(name);
            let mut out = vec![0u32; 4096];
            let n = unsafe { cpp::ref_audio_event(cname.as_ptr(), owner, id, pos.as_ptr(), c_int::from(logical), player_index, n_ops, ops, out.as_mut_ptr(), out.len() as c_int) } as usize;
            out.truncate(n);
            states.cmp(&a, &out, || format!("case {case} event {ev}: {name} owner {owner} id {id} logical {logical} ops {ops:#x}/{n_ops}"));
        }
        let mut words = Vec::new();
        for which in [0, 2] {
            let mut s = [0u32; 6];
            unsafe { cpp::ref_get_state(which, s.as_mut_ptr()) };
            words.extend(s);
        }
        let mine: Vec<u32> = r.logic.iter().chain(r.audio.iter()).copied().collect();
        rng.cmp(&mine, &words, || format!("case {case}"));
    }
    println!("COVERAGE audio events: {cases} cases, {localized} calls with a localized file, {logical_events} logical events, {logic_draws} logic draws");
    states.finish();
    rng.finish();
    assert!(localized > 0);
    assert!(logic_draws > 0);
}
