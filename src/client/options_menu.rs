// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/UserPreferences.h (class OptionPreferences)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/OptionsMenu.cpp (the file's statics,
//     enum Detail, OptionPreferences (every method), setDefaults, saveOptions, DestroyOptionsLayout,
//     showAdvancedOptions, acceptAdvancedOptions, cancelAdvancedOptions, OptionsMenuInit, OptionsMenuShutdown,
//     OptionsMenuUpdate, OptionsMenuInput, OptionsMenuSystem)
//   GeneralsMD/Code/GameEngine/Source/Common/GlobalData.cpp (GlobalData::parseGameDataDefinition's "override INI
//     values with user preferences" tail)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The options screen (GitHub #88): `Menus/OptionsMenu.wnd`'s callbacks and `OptionPreferences` (`Options.ini`).
//!
//! # As upstream reads it
//!
//! **`OptionPreferences`** is `Options.ini` ([`crate::client::user_preferences`]); each getter answers the global
//! data's (or the audio settings') value when its key is missing, and reads `yes` (any case) as true, numbers with
//! `atoi` / `atof`, clamped where noted. The global data takes them once at start (`parseGameDataDefinition`'s tail:
//! [`apply_option_preferences`]).
//!
//! **Init** (the main menu's *Options* runs it on the shell's options layout) makes the preference object if there is
//! none, finds the screen's controls, fills the IP address lists (`IPEnumeration`), the HTTP proxy (the registry), the
//! firewall port, the three anti-aliasing modes, the display modes (`TheDisplay`), the four detail levels (`GUI:High`
//! .. `GUI:Custom`, the LOD manager's level selected), the advanced display controls from the global data, the
//! language filter, replay camera and scroll-anchor check boxes from the preferences, the mouse and scroll controls
//! from the global data, the three volume sliders and the gamma slider from the preferences; shows the screen, focuses
//! and makes modal its parent, and in a game (not the shell's) or online disables the controls that cannot change
//! there. **System**: choosing *Custom* detail shows the advanced display window (*Accept* / *Back* there hide it,
//! *Back* restoring the detail); *Accept* stores every control into the global data and the preferences
//! (`saveOptions`: texture reduction and shadows for custom detail, the LOD level, the display mode, the IP addresses,
//! the proxy, the firewall port, anti-aliasing, the mouse mode, the scroll speed, the volumes, applied to the audio,
//! and the gamma, applied to the display), writes `Options.ini` and closes the screen; *Back* closes it without
//! writing; *Defaults* sets the controls to the defaults (`setDefaults`, nothing stored); the scroll-anchor and replay
//! camera check boxes store at once into the in-game UI or the global data and the preferences; *Firewall Refresh*
//! forgets the detected firewall; *Keyboard Options* pushes `Menus/KeyboardOptionsMenu.wnd`. **Input**: Escape's
//! release presses *Back*.
//!
//! # Upstream quirks that are kept
//!
//! - `getLANIPAddress`, `getOnlineIPAddress` and Init read `(*this)["IPAddress"]`, `["GameSpyIPAddress"]`,
//!   `["AntiAliasing"]`, `["Resolution"]`, `["LanguageFilter"]`, `["DrawScrollAnchor"]`, `["MoveScrollAnchor"]` with
//!   `operator[]`, which inserts an empty value for a missing key (written as `Key = `, skipped on the next load).
//! - Init accepts an anti-aliasing mode of 3 (`val > NUM_ALIASING_MODES`, not `>=`): the combo box gets a selection
//!   past its three entries.
//! - Init, with a saved display mode the display lacks, sets `selectedXRes = 800; selectedXRes = 600;` (the second
//!   was meant for Y): the global data's resolution becomes 600 by the saved height.
//! - `getExtraAnimationsDisabled` answers the opposite of the key (`yes` is "not disabled").
//! - `getTextureReduction` clamps only above (2); `getParticleCap` only below (100); `getGammaValue` is `atoi`, not
//!   `atof`; `getFirewallPortAllocationDelta` truncates `atoi` to a `Short`.
//! - The pref object is kept between openings unless *Back* or *Accept* deleted it; the statics keep the controls of
//!   the last opening (only the two IP combo boxes are cleared).
//! - `dispChanged` is set by a display change and never cleared here.
//! - Escape sends `GBM_SELECTED` with the *Back* button looked up under the window that got the key.
//!
//! # Where upstream is undefined (recorded with [`GameWindowManager::set_undefined`])
//!
//! - A control the file lacks: the check box and slider readers dereference it.
//! - `saveOptions`' `index` read before any combo box wrote it (every combo box missing).
//! - What the preference reads leave undefined ([`crate::client::user_preferences::PrefsUndefined`]).
//! - A system message after *Accept* or *Back* deleted the pref object (`(*pref)[..]` on `NULL`).
//!
//! # Where this differs from upstream
//!
//! The client has some of what the options set and not the rest; what it lacks is the host's to answer or record, and
//! is never made up here ([`OptionsHost`]):
//! - **Applied by the client:** the volumes (`TheAudio->setVolume`: [`OptionsHost::set_volume`]), the keyboard scroll
//!   factor and the alternate mouse mode (the client's copy of the global data, which its camera and command
//!   translation read: `client/src/shell.rs`).
//! - **Kept, not applied (recorded gaps):** the display modes, the resolution change and the dialog that confirms it
//!   (`TheDisplay`, `DoResolutionDialog`, the shell's re-creation), the gamma (`TheDisplay->setGamma`), ~~the LOD manager
//!   (`TheGameLODManager`: its level is the host's, GameLOD.cpp is not translated)~~ **Corrected 2026-11-12 (#160):**
//!   the LOD manager is translated (`crate::client::game_lod`, the shell's `ShellGui::lod`): its levels are applied to the
//!   client's copy of the global data, and the particle cap and the draw modules' minimum LOD read it; texture reduction
//!   (`TheGameClient->adjustLOD`), the shadows, cloud map, light map, soft water, trees, heat effects, building markers
//!   and particle cap (the Bevy renderer's own), anti-aliasing, the IP addresses (`IPEnumeration`: the client has no
//!   network), the firewall and send delay, the HTTP proxy (the registry, `ghttpSetProxy`), the version label
//!   (`GetRegistryVersion`), the RMB scroll anchors (the in-game UI's), the replay camera, the language filter,
//!   retaliation and double-click attack move (the client has no such input yet).
//! - `TheGameSpyInfo` is `NULL` and no GameSpy overlay is open (the client has no online service);
//!   `TheVersion->showFullVersion()` is false (the release's default).
//!
//! # Representation
//!
//! [`OptionsMenu`]: the file's statics (`pref`, the controls and their ids, `ignoreSelected`, `dispChanged`,
//! `oldDispSettings` / `newDispSettings`, `OptionsLayout`, the system callback's button ids). The controls are one
//! table by [`CONTROL_NAMES`]. [`OptionPreferences`] holds a [`UserPreferences`].

use std::ops::{Deref, DerefMut};

use crate::client::audio_info::{AudioSettings, OptionPrefs};
use crate::client::control_bar_popup::{format_wide, FormatArg};
use crate::client::gadget::{check_box_is_checked, check_box_set_checked, static_text_set_text};
use crate::client::gadget_combo_box as cb;
use crate::client::gadget_slider::{slider_get_position, slider_rt, slider_set_position};
use crate::client::gadget_text_entry as te;
use crate::client::game_audio::{AUDIO_AFFECT_MUSIC, AUDIO_AFFECT_SOUND, AUDIO_AFFECT_SOUND_3D, AUDIO_AFFECT_SPEECH, AUDIO_AFFECT_SYSTEM_SETTING};
use crate::client::main_menu::{MenuHost, DIFFICULTY_EASY, DIFFICULTY_HARD};
use crate::client::shell;
use crate::client::user_preferences::{atof, atoi, report_undefined, HostUserData, PrefsUndefined, UserDataFiles, UserPreferences};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::common::ascii_string::AsciiString;
use crate::common::base_type::real_to_int;
use crate::common::global_data::GlobalData;
use crate::common::ini::game_make_color;
use crate::common::recorder::game_mode;

/// `enum Detail`.
pub const HIGHDETAIL: i32 = 0;
pub const MEDIUMDETAIL: i32 = 1;
pub const LOWDETAIL: i32 = 2;
pub const CUSTOMDETAIL: i32 = 3;

/// `StaticGameLODLevel`.
pub const STATIC_GAME_LOD_UNKNOWN: i32 = -1;
pub const STATIC_GAME_LOD_LOW: i32 = 0;
pub const STATIC_GAME_LOD_MEDIUM: i32 = 1;
pub const STATIC_GAME_LOD_HIGH: i32 = 2;
pub const STATIC_GAME_LOD_CUSTOM: i32 = 3;

/// Init's `enum AliasingMode`'s `NUM_ALIASING_MODES`.
pub const NUM_ALIASING_MODES: i32 = 3;

/// `FirewallHelperClass::FIREWALL_TYPE_UNKNOWN`.
pub const FIREWALL_TYPE_UNKNOWN: u32 = 0;

/// `SHELL_SCRIPT_HOOK_OPTIONS_OPENED`, `SHELL_SCRIPT_HOOK_OPTIONS_CLOSED`: ~~the enum's names as strings~~ **Corrected
/// 2026-11-11, #142:** the hooks themselves; `SignalUIInteraction` passes `TheShellHookNames[hook]` (#140).
const HOOK_OPENED: i32 = crate::client::shell_hooks::SHELL_SCRIPT_HOOK_OPTIONS_OPENED;
const HOOK_CLOSED: i32 = crate::client::shell_hooks::SHELL_SCRIPT_HOOK_OPTIONS_CLOSED;

// ------------------------------------------------------------------------------------------------------------------
// OptionPreferences
// ------------------------------------------------------------------------------------------------------------------

/// `class OptionPreferences : public UserPreferences`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OptionPreferences {
    pub prefs: UserPreferences,
}

impl Deref for OptionPreferences {
    type Target = UserPreferences;
    fn deref(&self) -> &UserPreferences {
        &self.prefs
    }
}

impl DerefMut for OptionPreferences {
    fn deref_mut(&mut self) -> &mut UserPreferences {
        &mut self.prefs
    }
}

/// `%d.%d.%d.%d` of an address (the most significant byte first).
fn ip_string(ip: u32) -> Vec<u8> {
    format!("{}.{}.{}.{}", (ip & 0xff00_0000) >> 24, (ip & 0xff_0000) >> 16, (ip & 0xff00) >> 8, ip & 0xff).into_bytes()
}

/// `sscanf(s, "%d%d", &x, &y) == 2`: two decimal numbers, blanks before each.
fn scan_two_ints(s: &[u8], undefined: &mut Vec<PrefsUndefined>) -> Option<(i32, i32)> {
    let mut i = 0;
    let mut one = |i: &mut usize| -> Option<i32> {
        while *i < s.len() && matches!(s[*i], b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
            *i += 1;
        }
        let start = *i;
        if matches!(s.get(*i), Some(b'-') | Some(b'+')) {
            *i += 1;
        }
        let d = s[*i..].iter().take_while(|c| c.is_ascii_digit()).count();
        if d == 0 {
            return None;
        }
        *i += d;
        Some(atoi(&s[start..*i], undefined))
    };
    let x = one(&mut i)?;
    let y = one(&mut i)?;
    Some((x, y))
}

impl OptionPreferences {
    /// `OptionPreferences()`: `load("Options.ini")`.
    pub fn load_from<F: UserDataFiles + ?Sized>(fs: &mut F) -> OptionPreferences {
        let mut p = OptionPreferences::default();
        p.prefs.load(fs, b"Options.ini");
        p
    }

    /// `OptionPreferences()` over the shell's host, what the read left undefined recorded.
    pub fn new<H: MenuHost + ?Sized>(wm: &mut GameWindowManager, h: &mut H) -> OptionPreferences {
        let mut p = OptionPreferences::load_from(&mut HostUserData(h));
        report_undefined(wm, &mut p.prefs);
        p
    }

    fn found(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.prefs.map.get(key).cloned()
    }

    /// The getters' `stricmp(it->second.str(), "yes") == 0` (`None`: no entry).
    fn yes(&self, key: &[u8]) -> Option<bool> {
        self.found(key).map(|v| v.eq_ignore_ascii_case(b"yes"))
    }

    fn int(&mut self, key: &[u8]) -> Option<i32> {
        let v = self.found(key)?;
        Some(atoi(&v, &mut self.prefs.undefined))
    }

    /// `getCampaignDifficulty()`: clamped to easy .. hard, or `TheScriptEngine->getGlobalDifficulty()`.
    pub fn get_campaign_difficulty(&mut self, global_difficulty: i32) -> i32 {
        match self.int(b"CampaignDifficulty") {
            None => global_difficulty,
            Some(f) => f.clamp(DIFFICULTY_EASY, DIFFICULTY_HARD),
        }
    }

    /// `setCampaignDifficulty(diff)`.
    pub fn set_campaign_difficulty(&mut self, diff: i32) {
        self.prefs.set_ascii_string(b"CampaignDifficulty", format!("{diff}").as_bytes());
    }

    /// `getLANIPAddress()` / `getOnlineIPAddress()`: the enumerated address whose text is the entry (`compareNoCase`),
    /// else `TheGlobalData->m_defaultIP`. The entry is read with `operator[]` (inserted empty when missing).
    fn get_ip(&mut self, key: &[u8], ips: &[(Vec<u8>, u32)], default_ip: u32) -> u32 {
        let selected = self.prefs.index(key);
        ips.iter().find(|(s, _)| selected.eq_ignore_ascii_case(s)).map_or(default_ip, |(_, ip)| *ip)
    }

    /// `getLANIPAddress()`.
    pub fn get_lan_ip_address(&mut self, ips: &[(Vec<u8>, u32)], default_ip: u32) -> u32 {
        self.get_ip(b"IPAddress", ips, default_ip)
    }

    /// `getOnlineIPAddress()`.
    pub fn get_online_ip_address(&mut self, ips: &[(Vec<u8>, u32)], default_ip: u32) -> u32 {
        self.get_ip(b"GameSpyIPAddress", ips, default_ip)
    }

    /// `setLANIPAddress(AsciiString)`.
    pub fn set_lan_ip_address_str(&mut self, ip: &[u8]) {
        self.prefs.set_ascii_string(b"IPAddress", ip);
    }

    /// `setLANIPAddress(UnsignedInt)`.
    pub fn set_lan_ip_address(&mut self, ip: u32) {
        self.prefs.set_ascii_string(b"IPAddress", &ip_string(ip));
    }

    /// `setOnlineIPAddress(AsciiString)`.
    pub fn set_online_ip_address_str(&mut self, ip: &[u8]) {
        self.prefs.set_ascii_string(b"GameSpyIPAddress", ip);
    }

    /// `setOnlineIPAddress(UnsignedInt)`.
    pub fn set_online_ip_address(&mut self, ip: u32) {
        self.prefs.set_ascii_string(b"GameSpyIPAddress", &ip_string(ip));
    }

    /// `getAlternateMouseModeEnabled()`.
    pub fn get_alternate_mouse_mode_enabled(&self, g: &GlobalData) -> bool {
        self.yes(b"UseAlternateMouse").unwrap_or(g.use_alternate_mouse)
    }

    /// `getRetaliationModeEnabled()`.
    pub fn get_retaliation_mode_enabled(&self, g: &GlobalData) -> bool {
        self.yes(b"Retaliation").unwrap_or(g.client_retaliation_mode_enabled)
    }

    /// `getDoubleClickAttackMoveEnabled()`.
    pub fn get_double_click_attack_move_enabled(&self, g: &GlobalData) -> bool {
        self.yes(b"UseDoubleClickAttackMove").unwrap_or(g.double_click_attack_move)
    }

    /// `getScrollFactor()`: `ScrollFactor` clamped to 0 .. 100, in hundredths.
    pub fn get_scroll_factor(&mut self, g: &GlobalData) -> f32 {
        match self.int(b"ScrollFactor") {
            None => g.keyboard_default_scroll_factor,
            Some(f) => f.clamp(0, 100) as f32 / 100.0f32,
        }
    }

    /// `usesSystemMapDir()`.
    pub fn uses_system_map_dir(&self) -> bool {
        self.yes(b"UseSystemMapDir").unwrap_or(true)
    }

    /// `saveCameraInReplays()`.
    pub fn save_camera_in_replays(&self) -> bool {
        self.yes(b"SaveCameraInReplays").unwrap_or(true)
    }

    /// `useCameraInReplays()`.
    pub fn use_camera_in_replays(&self) -> bool {
        self.yes(b"UseCameraInReplays").unwrap_or(true)
    }

    /// `getIdealStaticGameDetail()`: `TheGameLODManager->getStaticGameLODIndex(entry)`, unknown without one.
    pub fn get_ideal_static_game_detail(&self, lod_index: impl Fn(&[u8]) -> i32) -> i32 {
        self.found(b"IdealStaticGameLOD").map_or(STATIC_GAME_LOD_UNKNOWN, |v| lod_index(&v))
    }

    /// `getStaticGameDetail()`: the index of the entry, or the LOD manager's level.
    pub fn get_static_game_detail(&self, static_lod_level: i32, lod_index: impl Fn(&[u8]) -> i32) -> i32 {
        self.found(b"StaticGameLOD").map_or(static_lod_level, |v| lod_index(&v))
    }

    /// `getSendDelay()`.
    pub fn get_send_delay(&self, g: &GlobalData) -> bool {
        self.yes(b"SendDelay").unwrap_or(g.firewall_send_delay)
    }

    /// `getFirewallBehavior()`: negative read as 0.
    pub fn get_firewall_behavior(&mut self, g: &GlobalData) -> i32 {
        match self.int(b"FirewallBehavior") {
            None => g.firewall_behavior as i32,
            Some(b) => b.max(0),
        }
    }

    /// `getFirewallPortAllocationDelta()`: `atoi` truncated to a `Short`.
    pub fn get_firewall_port_allocation_delta(&mut self, g: &GlobalData) -> i16 {
        self.int(b"FirewallPortAllocationDelta").map_or(g.firewall_port_allocation_delta, |d| d as i16)
    }

    /// `getFirewallPortOverride()`: 0 outside 0 .. 65535.
    pub fn get_firewall_port_override(&mut self, g: &GlobalData) -> u16 {
        match self.int(b"FirewallPortOverride") {
            None => g.firewall_port_override as u16,
            Some(o) if !(0..=65535).contains(&o) => 0,
            Some(o) => o as u16,
        }
    }

    /// `getFirewallNeedToRefresh()`: `TRUE` in any case.
    pub fn get_firewall_need_to_refresh(&self) -> bool {
        self.found(b"FirewallNeedToRefresh").is_some_and(|v| v.eq_ignore_ascii_case(b"TRUE"))
    }

    /// The audio's reading of the four volumes, the provider and the speakers (`OptionPreferences::getSoundVolume`,
    /// `get3DSoundVolume`, `getSpeechVolume`, `getMusicVolume`, `getPreferred3DProvider`, `getSpeakerType`, translated
    /// in [`OptionPrefs`]): each entry `atof`'d into it.
    pub fn audio_prefs(&self) -> OptionPrefs {
        let f = |k: &[u8]| self.found(k).map(|v| atof(&v) as f32);
        OptionPrefs {
            audio_provider_3d: self.found(b"3DAudioProvider"),
            speaker_type: self.found(b"SpeakerType"),
            sfx_volume: f(b"SFXVolume"),
            sfx_3d_volume: f(b"SFX3DVolume"),
            voice_volume: f(b"VoiceVolume"),
            music_volume: f(b"MusicVolume"),
        }
    }

    /// `getPreferred3DProvider()`.
    pub fn get_preferred_3d_provider(&self, s: &AudioSettings) -> Vec<u8> {
        self.audio_prefs().get_preferred_3d_provider(s)
    }

    /// `getSpeakerType()`.
    pub fn get_speaker_type(&self, s: &AudioSettings) -> Vec<u8> {
        self.audio_prefs().get_speaker_type(s)
    }

    /// `getSoundVolume()`.
    pub fn get_sound_volume(&self, s: &AudioSettings) -> f32 {
        self.audio_prefs().get_sound_volume(s)
    }

    /// `get3DSoundVolume()`.
    pub fn get_3d_sound_volume(&self, s: &AudioSettings) -> f32 {
        self.audio_prefs().get_3d_sound_volume(s)
    }

    /// `getSpeechVolume()`.
    pub fn get_speech_volume(&self, s: &AudioSettings) -> f32 {
        self.audio_prefs().get_speech_volume(s)
    }

    /// `getMusicVolume()`.
    pub fn get_music_volume(&self, s: &AudioSettings) -> f32 {
        self.audio_prefs().get_music_volume(s)
    }

    /// `getCloudShadowsEnabled()`.
    pub fn get_cloud_shadows_enabled(&self, g: &GlobalData) -> bool {
        self.yes(b"UseCloudMap").unwrap_or(g.use_cloud_map)
    }

    /// `getLightmapEnabled()`.
    pub fn get_lightmap_enabled(&self, g: &GlobalData) -> bool {
        self.yes(b"UseLightMap").unwrap_or(g.use_light_map)
    }

    /// `getSmoothWaterEnabled()`.
    pub fn get_smooth_water_enabled(&self, g: &GlobalData) -> bool {
        self.yes(b"ShowSoftWaterEdge").unwrap_or(g.show_soft_water_edge)
    }

    /// `getTreesEnabled()`.
    pub fn get_trees_enabled(&self, g: &GlobalData) -> bool {
        self.yes(b"ShowTrees").unwrap_or(g.use_trees)
    }

    /// `getExtraAnimationsDisabled()`: `yes` is false (not disabled); without the entry `m_useDrawModuleLOD`.
    pub fn get_extra_animations_disabled(&self, g: &GlobalData) -> bool {
        self.yes(b"ExtraAnimations").map_or(g.use_draw_module_lod, |y| !y)
    }

    /// `getUseHeatEffects()`.
    pub fn get_use_heat_effects(&self, g: &GlobalData) -> bool {
        self.yes(b"HeatEffects").unwrap_or(g.use_heat_effects)
    }

    /// `getDynamicLODEnabled()`.
    pub fn get_dynamic_lod_enabled(&self, g: &GlobalData) -> bool {
        self.yes(b"DynamicLOD").unwrap_or(g.enable_dynamic_lod)
    }

    /// `getFPSLimitEnabled()`.
    pub fn get_fps_limit_enabled(&self, g: &GlobalData) -> bool {
        self.yes(b"FPSLimit").unwrap_or(g.use_fps_limit)
    }

    /// `get3DShadowsEnabled()`.
    pub fn get_3d_shadows_enabled(&self, g: &GlobalData) -> bool {
        self.yes(b"UseShadowVolumes").unwrap_or(g.use_shadow_volumes)
    }

    /// `get2DShadowsEnabled()`.
    pub fn get_2d_shadows_enabled(&self, g: &GlobalData) -> bool {
        self.yes(b"UseShadowDecals").unwrap_or(g.use_shadow_decals)
    }

    /// `getBuildingOcclusionEnabled()`.
    pub fn get_building_occlusion_enabled(&self, g: &GlobalData) -> bool {
        self.yes(b"BuildingOcclusion").unwrap_or(g.enable_behind_building_markers)
    }

    /// `getParticleCap()`: at least 100.
    pub fn get_particle_cap(&mut self, g: &GlobalData) -> i32 {
        self.int(b"MaxParticleCount").map_or(g.max_particle_count, |f| f.max(100))
    }

    /// `getTextureReduction()`: -1 (unknown) without the entry; at most 2.
    pub fn get_texture_reduction(&mut self) -> i32 {
        self.int(b"TextureReduction").map_or(-1, |f| f.min(2))
    }

    /// `getGammaValue()`: `(Real)atoi`, 50 without the entry.
    pub fn get_gamma_value(&mut self) -> f32 {
        self.int(b"Gamma").map_or(50.0f32, |g| g as f32)
    }

    /// `getResolution(&x, &y)`: the global data's, unless `Resolution` holds two numbers.
    pub fn get_resolution(&mut self, g: &GlobalData) -> (i32, i32) {
        let def = (g.x_resolution, g.y_resolution);
        let Some(v) = self.found(b"Resolution") else { return def };
        scan_two_ints(&v, &mut self.prefs.undefined).unwrap_or(def)
    }
}

/// The gamma slider's value `val` (0 .. 100) as the display's gamma, 0.6 .. 2.0 (`saveOptions`, and
/// `parseGameDataDefinition`'s tail, which leaves the gamma alone at exactly 50).
fn gamma_of(val: i32) -> Option<f32> {
    if val < 50 {
        if val <= 0 {
            Some(0.6f32)
        } else {
            Some(1.0f32 - 0.4f32 * (50 - val) as f32 / 50.0f32)
        }
    } else if val > 50 {
        Some(1.0f32 + 1.0f32 * (val - 50) as f32 / 50.0f32)
    } else {
        None
    }
}

/// `GlobalData::parseGameDataDefinition`'s tail, "override INI values with user preferences": the mouse modes, the
/// scroll factor, the default IP, the firewall, the replay camera, the gamma and the resolution from `Options.ini`.
/// `ips` is `IPEnumeration`'s list.
pub fn apply_option_preferences(g: &mut GlobalData, p: &mut OptionPreferences, ips: &[(Vec<u8>, u32)]) {
    g.use_alternate_mouse = p.get_alternate_mouse_mode_enabled(g);
    g.client_retaliation_mode_enabled = p.get_retaliation_mode_enabled(g);
    g.double_click_attack_move = p.get_double_click_attack_move_enabled(g);
    g.keyboard_scroll_factor = p.get_scroll_factor(g);
    g.default_ip = p.get_lan_ip_address(ips, g.default_ip);
    g.firewall_send_delay = p.get_send_delay(g);
    g.firewall_behavior = p.get_firewall_behavior(g) as u32;
    g.firewall_port_allocation_delta = p.get_firewall_port_allocation_delta(g);
    g.firewall_port_override = u32::from(p.get_firewall_port_override(g));
    g.save_camera_in_replay = p.save_camera_in_replays();
    g.use_camera_in_replay = p.use_camera_in_replays();
    // Int val = optionPref.getGammaValue();
    let val = p.get_gamma_value() as i32;
    if let Some(gamma) = gamma_of(val) {
        g.display_gamma = gamma;
    }
    let (x, y) = p.get_resolution(g);
    g.x_resolution = x;
    g.y_resolution = y;
}

// ------------------------------------------------------------------------------------------------------------------
// The screen
// ------------------------------------------------------------------------------------------------------------------

/// The controls Init looks up, by their names after `OptionsMenu.wnd:` (the table's indices are the `C_*` constants).
pub const CONTROL_NAMES: [&str; 36] = [
    "ComboBoxIP", "ComboBoxOnlineIP", "CheckAlternateMouse", "Retaliation", "CheckDoubleClickAttackMove", "SliderScrollSpeed", "ComboBoxAntiAliasing",
    "ComboBoxResolution", "ComboBoxDetail", "CheckLanguageFilter", "CheckSendDelay", "ButtonFirewallRefresh", "CheckBoxDrawAnchor", "CheckBoxMoveAnchor",
    "CheckBoxSaveCamera", "CheckBoxUseCamera", "SliderMusicVolume", "SliderSFXVolume", "SliderVoiceVolume", "SliderGamma", "WinAdvancedDisplayOptions",
    "ButtonAdvanceAccept", "ButtonAdvanceBack", "LowResSlider", "Check3DShadows", "Check2DShadows", "CheckCloudShadows", "CheckGroundLighting",
    "CheckSmoothWater", "CheckExtraAnimations", "CheckNoDynamicLOD", "CheckHeatEffects", "CheckUnlockFPS", "CheckBehindBuilding", "CheckShowProps",
    "ParticleCapSlider",
];
pub const C_LAN_IP: usize = 0;
pub const C_ONLINE_IP: usize = 1;
pub const C_ALT_MOUSE: usize = 2;
pub const C_RETALIATION: usize = 3;
pub const C_DOUBLE_CLICK: usize = 4;
pub const C_SCROLL: usize = 5;
pub const C_ANTI_ALIAS: usize = 6;
pub const C_RESOLUTION: usize = 7;
pub const C_DETAIL: usize = 8;
pub const C_LANGUAGE: usize = 9;
pub const C_SEND_DELAY: usize = 10;
pub const C_FIREWALL_REFRESH: usize = 11;
pub const C_DRAW_ANCHOR: usize = 12;
pub const C_MOVE_ANCHOR: usize = 13;
pub const C_SAVE_CAMERA: usize = 14;
pub const C_USE_CAMERA: usize = 15;
pub const C_MUSIC: usize = 16;
pub const C_SFX: usize = 17;
pub const C_VOICE: usize = 18;
pub const C_GAMMA: usize = 19;
pub const C_ADVANCED: usize = 20;
pub const C_ADV_ACCEPT: usize = 21;
pub const C_ADV_CANCEL: usize = 22;
pub const C_TEXTURE: usize = 23;
pub const C_3D_SHADOWS: usize = 24;
pub const C_2D_SHADOWS: usize = 25;
pub const C_CLOUD: usize = 26;
pub const C_GROUND: usize = 27;
pub const C_WATER: usize = 28;
pub const C_EXTRA_ANIM: usize = 29;
pub const C_NO_DYNAMIC_LOD: usize = 30;
pub const C_HEAT: usize = 31;
pub const C_UNLOCK_FPS: usize = 32;
pub const C_BUILDING: usize = 33;
pub const C_PROPS: usize = 34;
pub const C_PARTICLE: usize = 35;

/// `DisplaySettings` (Saad's resolution confirmation).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DisplaySettings {
    pub x_res: i32,
    pub y_res: i32,
    pub bit_depth: i32,
    pub windowed: bool,
}

/// OptionsMenu.cpp's statics and globals (`NAMEKEY_INVALID` is 0, `NULL` is `None`).
#[derive(Clone, Debug, PartialEq)]
pub struct OptionsMenu {
    /// `static OptionPreferences *pref`.
    pub pref: Option<OptionPreferences>,
    /// The controls' ids (`comboBoxLANIPID`, ...), by [`CONTROL_NAMES`].
    pub ids: [i32; 36],
    /// The controls (`comboBoxLANIP`, ...).
    pub ctl: [Option<WinId>; 36],
    pub ignore_selected: bool,
    /// `OptionsLayout`.
    pub layout: Option<usize>,
    pub disp_changed: bool,
    pub old_disp_settings: DisplaySettings,
    pub new_disp_settings: DisplaySettings,
    /// `OptionsMenuSystem`'s statics: `buttonBack`, `buttonDefaults`, `buttonAccept`, `buttonKeyboardOptionsMenu`
    /// (`buttonReplayMenu` is never set or read).
    pub button_back: i32,
    pub button_defaults: i32,
    pub button_accept: i32,
    pub button_keyboard_options_menu: i32,
}

impl Default for OptionsMenu {
    /// The statics' initial values.
    fn default() -> Self {
        OptionsMenu {
            pref: None,
            ids: [0; 36],
            ctl: [None; 36],
            ignore_selected: false,
            layout: None,
            disp_changed: false,
            old_disp_settings: DisplaySettings::default(),
            new_disp_settings: DisplaySettings::default(),
            button_back: 0,
            button_defaults: 0,
            button_accept: 0,
            button_keyboard_options_menu: 0,
        }
    }
}

/// What the options screen needs besides the main menu's host: what OptionsMenu.cpp reaches outside itself.
pub trait OptionsHost: MenuHost {
    /// The file's statics.
    fn options_menu(&mut self) -> &mut OptionsMenu;
    /// `TheWritableGlobalData` (the client's copy; the logic's own is never written).
    fn global_data(&mut self) -> &mut GlobalData;
    /// `TheAudio->getAudioSettings()`.
    fn audio_settings(&self) -> AudioSettings;
    /// `TheAudio->setVolume(volume, affect)`.
    fn set_volume(&mut self, volume: f32, affect: u32);
    /// `TheGameEngine->getQuitting()`.
    fn quitting(&self) -> bool {
        false
    }
    /// `TheGameLogic->getGameMode()`.
    fn game_mode(&self) -> i32 {
        game_mode::NONE
    }
    /// `IPEnumeration::getAddresses()`: each address's text and value, in order.
    fn ip_addresses(&mut self) -> Vec<(Vec<u8>, u32)>;
    /// `GetRegistryVersion()`.
    fn registry_version(&mut self) -> u32;
    /// `GetStringFromRegistry("", "Proxy", ...)`.
    fn registry_proxy(&mut self) -> Vec<u8>;
    /// `SetStringInRegistry("", "Proxy", ..)` and `ghttpSetProxy(..)`.
    fn set_proxy(&mut self, proxy: &[u8]);
    /// `TheGameLODManager->getStaticLODLevel()`.
    fn static_lod_level(&mut self) -> i32;
    /// `TheGameLODManager->setStaticLODLevel(level)`: whether it changed.
    fn set_static_lod_level(&mut self, level: i32) -> bool;
    /// `TheGameLODManager->findStaticLODLevel()`.
    fn find_static_lod_level(&mut self) -> i32;
    /// `TheGameLODManager->getCurrentTextureReduction()`.
    fn current_texture_reduction(&mut self) -> i32;
    /// `TheGameLODManager->getRecommendedTextureReduction()`.
    fn recommended_texture_reduction(&mut self) -> i32;
    /// `TheGameLODManager->getStaticGameLODLevelName(level)`.
    fn static_game_lod_level_name(&mut self, level: i32) -> Vec<u8>;
    /// `TheGameClient->adjustLOD(adj)`.
    fn adjust_lod(&mut self, adj: i32);
    /// `TheDisplay->getDisplayModeCount()` and `getDisplayModeDescription(i)`: each mode's width, height, bit depth.
    fn display_modes(&mut self) -> Vec<(i32, i32, i32)>;
    /// `TheDisplay->getWidth()`, `getHeight()`, `getBitDepth()`, `getWindowed()`.
    fn display_settings(&mut self) -> DisplaySettings;
    /// `TheDisplay->setDisplayMode(x, y, bits, windowed)`: whether it changed.
    fn set_display_mode(&mut self, x: i32, y: i32, bits: i32, windowed: bool) -> bool;
    /// What a display change does after it (`headerNotifyResolutionChange`, `mouseNotifyResolutionChange`, the shell
    /// made anew and `Menus/MainMenu.wnd` pushed, `recreateControlBar`).
    fn resolution_changed(&mut self, wm: &mut GameWindowManager);
    /// `TheDisplay->setGamma(gamma, 0, 1, FALSE)`.
    fn set_gamma(&mut self, gamma: f32);
    /// `TheInGameUI->getDrawRMBScrollAnchor()` / `setDrawRMBScrollAnchor`.
    fn draw_rmb_scroll_anchor(&mut self) -> &mut bool;
    /// `TheInGameUI->getMoveRMBScrollAnchor()` / `setMoveRMBScrollAnchor`.
    fn move_rmb_scroll_anchor(&mut self) -> &mut bool;
    /// `destroyQuitMenu()`.
    fn options_destroy_quit_menu(&mut self, wm: &mut GameWindowManager);
    /// `DoResolutionDialog()`.
    fn do_resolution_dialog(&mut self, wm: &mut GameWindowManager);
}

fn ctl<H: OptionsHost>(h: &mut H, c: usize) -> Option<WinId> {
    h.options_menu().ctl[c]
}

/// `GadgetCheckBoxSetChecked(control, checked)` (a missing control is dereferenced: undefined).
fn set_checked<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H, c: usize, checked: bool) {
    match ctl(h, c) {
        Some(w) => check_box_set_checked(wm, h, w, checked),
        None => wm.set_undefined("OptionsMenu: GadgetCheckBoxSetChecked of a missing control"),
    }
}

/// `GadgetCheckBoxIsChecked(control)`.
fn is_checked<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H, c: usize) -> bool {
    match ctl(h, c) {
        Some(w) => check_box_is_checked(wm, w),
        None => {
            wm.set_undefined("OptionsMenu: GadgetCheckBoxIsChecked of a missing control");
            false
        }
    }
}

/// `GadgetSliderSetPosition(control, pos)` (a message: nothing for a missing control).
fn set_slider<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H, c: usize, pos: i32) {
    if let Some(w) = ctl(h, c) {
        slider_set_position(wm, h, w, pos);
    }
}

/// `GadgetSliderGetPosition(control)` (-1 without slider data; a missing control is dereferenced: undefined).
fn get_slider<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H, c: usize) -> i32 {
    match ctl(h, c) {
        Some(w) => slider_get_position(wm, w),
        None => {
            wm.set_undefined("OptionsMenu: GadgetSliderGetPosition of a missing control");
            -1
        }
    }
}

/// `GadgetComboBoxSetSelectedPos(control, index)` (nothing for a missing control).
fn set_selected<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H, c: usize, index: i32) {
    if let Some(w) = ctl(h, c) {
        cb::set_selected_pos(wm, h, w, index, false);
    }
}

/// `GadgetComboBoxGetSelectedPos(control, &index)`: `index` is left as it was for a missing control (`None`: never
/// written, i.e. uninitialised).
fn get_selected<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H, c: usize, index: &mut Option<i32>) {
    if let Some(w) = ctl(h, c) {
        *index = Some(cb::get_selected_pos(wm, h, w, index.unwrap_or(-1)));
    }
}

/// The LOD level's detail entry (`switch (level)`; any other level is `DEBUG_ASSERTCRASH`: nothing in the release).
fn select_detail_for_level<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H, level: i32) {
    let pos = match level {
        STATIC_GAME_LOD_LOW => LOWDETAIL,
        STATIC_GAME_LOD_MEDIUM => MEDIUMDETAIL,
        STATIC_GAME_LOD_HIGH => HIGHDETAIL,
        STATIC_GAME_LOD_CUSTOM => CUSTOMDETAIL,
        _ => return,
    };
    set_selected(wm, h, C_DETAIL, pos);
}

/// `(*pref)[key] = value` (the pref object deleted: undefined).
fn pref_set<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H, key: &[u8], value: &[u8]) {
    match h.options_menu().pref.as_mut() {
        Some(p) => p.set_ascii_string(key, value),
        None => wm.set_undefined("OptionsMenu: (*pref)[..] with pref NULL"),
    }
}

fn yes_no(b: bool) -> &'static [u8] {
    if b {
        b"yes"
    } else {
        b"no"
    }
}

/// `setDefaults()`.
pub fn set_defaults<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H) {
    set_checked(wm, h, C_LANGUAGE, true);
    set_checked(wm, h, C_SEND_DELAY, false);
    let env = h.env();
    if !env.in_game || env.in_shell_game {
        let level = h.find_static_lod_level();
        select_detail_for_level(wm, h, level);
    }
    // `(isInGame() == FALSE) || (isInShellGame() == TRUE) && !TheGameSpyInfo` (&& binds first; TheGameSpyInfo NULL)
    if !env.in_game || env.in_shell_game {
        let modes = h.display_modes();
        let default_res_index = modes.iter().position(|&(x, y, _)| x == 800 && y == 600).map_or(0, |i| i as i32);
        set_selected(wm, h, C_RESOLUTION, default_res_index);
    }
    set_checked(wm, h, C_ALT_MOUSE, false);
    set_checked(wm, h, C_RETALIATION, true);
    set_checked(wm, h, C_DOUBLE_CLICK, false);
    let scroll_pos = (h.global_data().keyboard_default_scroll_factor * 100.0f32) as i32;
    set_slider(wm, h, C_SCROLL, scroll_pos);
    let a = h.audio_settings();
    set_slider(wm, h, C_MUSIC, real_to_int(a.default_music_volume * 100.0f32));
    let max_volume = if a.default_sound_volume > a.default_3d_sound_volume { a.default_sound_volume } else { a.default_3d_sound_volume };
    set_slider(wm, h, C_SFX, real_to_int(max_volume * 100.0f32));
    set_slider(wm, h, C_VOICE, real_to_int(a.default_speech_volume * 100.0f32));
    // GadgetSliderGetMinMax(sliderGamma, ...) (a missing slider is dereferenced)
    match ctl(h, C_GAMMA).and_then(|w| slider_rt(wm, w)) {
        Some(rt) => set_slider(wm, h, C_GAMMA, (rt.max_val - rt.min_val) / 2 + rt.min_val),
        None => wm.set_undefined("OptionsMenu setDefaults: GadgetSliderGetMinMax of a missing gamma slider"),
    }
    if !env.in_game || env.in_shell_game {
        let txt_fact = h.recommended_texture_reduction();
        set_slider(wm, h, C_TEXTURE, 2 - txt_fact);
        let g = h.global_data().clone();
        set_checked(wm, h, C_3D_SHADOWS, g.use_shadow_volumes);
        set_checked(wm, h, C_2D_SHADOWS, g.use_shadow_decals);
        set_checked(wm, h, C_CLOUD, g.use_cloud_map);
        set_checked(wm, h, C_GROUND, g.use_light_map);
        set_checked(wm, h, C_WATER, g.show_soft_water_edge);
        set_checked(wm, h, C_EXTRA_ANIM, !g.use_draw_module_lod);
        set_checked(wm, h, C_NO_DYNAMIC_LOD, !g.enable_dynamic_lod);
        set_checked(wm, h, C_UNLOCK_FPS, !g.use_fps_limit);
        set_checked(wm, h, C_HEAT, g.use_heat_effects);
        set_checked(wm, h, C_BUILDING, g.enable_behind_building_markers);
        set_slider(wm, h, C_PARTICLE, g.max_particle_count);
        set_checked(wm, h, C_PROPS, g.use_trees);
    }
}

/// `saveOptions()`.
pub fn save_options<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H) {
    let mut index: Option<i32> = None;
    // language filter
    let lang = is_checked(wm, h, C_LANGUAGE);
    h.global_data().language_filter_pref = lang;
    pref_set(wm, h, b"LanguageFilter", if lang { b"true" } else { b"false" });
    // send delay
    let delay = is_checked(wm, h, C_SEND_DELAY);
    h.global_data().firewall_send_delay = delay;
    pref_set(wm, h, b"SendDelay", yes_no(delay));
    // custom game detail
    get_selected(wm, h, C_DETAIL, &mut index);
    if index.is_none() {
        wm.set_undefined("OptionsMenu saveOptions: index read uninitialised (no ComboBoxDetail)");
    }
    if index == Some(CUSTOMDETAIL) {
        let val = 2 - get_slider(wm, h, C_TEXTURE);
        pref_set(wm, h, b"TextureReduction", format!("{val}").as_bytes());
        let current = h.global_data().texture_reduction_factor;
        if current != val {
            h.adjust_lod(val - current);
        }
        let flags: [(usize, &[u8]); 5] = [(C_3D_SHADOWS, b"UseShadowVolumes"), (C_2D_SHADOWS, b"UseShadowDecals"), (C_CLOUD, b"UseCloudMap"), (C_GROUND, b"UseLightMap"), (C_WATER, b"ShowSoftWaterEdge")];
        for (c, key) in flags {
            let on = is_checked(wm, h, c);
            let g = h.global_data();
            match c {
                C_3D_SHADOWS => g.use_shadow_volumes = on,
                C_2D_SHADOWS => g.use_shadow_decals = on,
                C_CLOUD => g.use_cloud_map = on,
                C_GROUND => g.use_light_map = on,
                _ => g.show_soft_water_edge = on,
            }
            pref_set(wm, h, key, yes_no(on));
        }
        let extra = is_checked(wm, h, C_EXTRA_ANIM);
        h.global_data().use_draw_module_lod = !extra;
        h.global_data().use_tree_sway = extra;
        pref_set(wm, h, b"ExtraAnimations", yes_no(extra));
        let dynamic = !is_checked(wm, h, C_NO_DYNAMIC_LOD);
        h.global_data().enable_dynamic_lod = dynamic;
        pref_set(wm, h, b"DynamicLOD", yes_no(dynamic));
        let heat = is_checked(wm, h, C_HEAT);
        h.global_data().use_heat_effects = heat;
        pref_set(wm, h, b"HeatEffects", yes_no(heat));
        // ("Never write this out": the FPS limit)
        let building = is_checked(wm, h, C_BUILDING);
        h.global_data().enable_behind_building_markers = building;
        pref_set(wm, h, b"BuildingOcclusion", yes_no(building));
        let trees = is_checked(wm, h, C_PROPS);
        h.global_data().use_trees = trees;
        pref_set(wm, h, b"ShowTrees", yes_no(trees));
        let val = get_slider(wm, h, C_PARTICLE);
        pref_set(wm, h, b"MaxParticleCount", format!("{val}").as_bytes());
        h.global_data().max_particle_count = val;
    }
    // LOD
    get_selected(wm, h, C_DETAIL, &mut index);
    let level_changed = match index {
        Some(HIGHDETAIL) => h.set_static_lod_level(STATIC_GAME_LOD_HIGH),
        Some(MEDIUMDETAIL) => h.set_static_lod_level(STATIC_GAME_LOD_MEDIUM),
        Some(LOWDETAIL) => h.set_static_lod_level(STATIC_GAME_LOD_LOW),
        Some(CUSTOMDETAIL) => h.set_static_lod_level(STATIC_GAME_LOD_CUSTOM),
        _ => false,
    };
    if level_changed {
        let level = h.static_lod_level();
        let name = h.static_game_lod_level_name(level);
        pref_set(wm, h, b"StaticGameLOD", &name);
    }
    // resolution
    get_selected(wm, h, C_RESOLUTION, &mut index);
    let old = h.display_settings();
    h.options_menu().old_disp_settings = old;
    let modes = h.display_modes();
    if let Some(i) = index.filter(|&i| i >= 0 && (i as usize) < modes.len()) {
        let (xres, yres, bit_depth) = modes[i as usize];
        let (gx, gy) = (h.global_data().x_resolution, h.global_data().y_resolution);
        if (gx != xres || gy != yres) && h.set_display_mode(xres, yres, bit_depth, old.windowed) {
            h.options_menu().disp_changed = true;
            h.global_data().x_resolution = xres;
            h.global_data().y_resolution = yres;
            let windowed = h.display_settings().windowed;
            h.options_menu().new_disp_settings = DisplaySettings { x_res: xres, y_res: yres, bit_depth, windowed };
            pref_set(wm, h, b"Resolution", format!("{xres} {yres}").as_bytes());
            h.resolution_changed(wm);
        }
    }
    // IP address (`GadgetComboBoxGetItemData` of a missing combo box is 0)
    get_selected(wm, h, C_LAN_IP, &mut index);
    if let Some(i) = index.filter(|&i| i >= 0) {
        let ip = ctl(h, C_LAN_IP).map_or(0, |w| cb::get_item_data(wm, h, w, i));
        h.global_data().default_ip = ip;
        match h.options_menu().pref.as_mut() {
            Some(p) => p.set_lan_ip_address(ip),
            None => wm.set_undefined("OptionsMenu: pref->setLANIPAddress with pref NULL"),
        }
    }
    get_selected(wm, h, C_ONLINE_IP, &mut index);
    if let Some(i) = index.filter(|&i| i >= 0) {
        let ip = ctl(h, C_ONLINE_IP).map_or(0, |w| cb::get_item_data(wm, h, w, i));
        match h.options_menu().pref.as_mut() {
            Some(p) => p.set_online_ip_address(ip),
            None => wm.set_undefined("OptionsMenu: pref->setOnlineIPAddress with pref NULL"),
        }
    }
    // HTTP proxy
    let proxy_id = h.name_to_key(b"OptionsMenu.wnd:TextEntryHTTPProxy") as i32;
    if let Some(entry) = wm.win_get_window_from_id(None, proxy_id) {
        let u = te::get_text(wm, h, Some(entry));
        let mut a = AsciiString::new();
        let _ = a.translate(&u);
        h.set_proxy(a.str());
    }
    // firewall port override
    let port_id = h.name_to_key(b"OptionsMenu.wnd:TextEntryFirewallPortOverride") as i32;
    if let Some(entry) = wm.win_get_window_from_id(None, port_id) {
        let u = te::get_text(wm, h, Some(entry));
        let mut a = AsciiString::new();
        let _ = a.translate(&u);
        let mut undefined = Vec::new();
        let mut o = atoi(a.str(), &mut undefined);
        if !undefined.is_empty() {
            wm.set_undefined("OptionsMenu saveOptions: atoi of the firewall port outside Int's range");
        }
        if !(0..=65535).contains(&o) {
            o = 0;
        }
        if h.global_data().firewall_port_override != o as u32 {
            h.global_data().firewall_port_override = o as u32;
            pref_set(wm, h, b"FirewallPortOverride", format!("{o}").as_bytes());
        }
    }
    // anti-aliasing
    get_selected(wm, h, C_ANTI_ALIAS, &mut index);
    if let Some(i) = index.filter(|&i| i >= 0) {
        if h.global_data().anti_alias_box_value != i {
            h.global_data().anti_alias_box_value = i;
            pref_set(wm, h, b"AntiAliasing", format!("{i}").as_bytes());
        }
    }
    // mouse mode
    let alt = is_checked(wm, h, C_ALT_MOUSE);
    h.global_data().use_alternate_mouse = alt;
    pref_set(wm, h, b"UseAlternateMouse", yes_no(alt));
    let ret = is_checked(wm, h, C_RETALIATION);
    h.global_data().client_retaliation_mode_enabled = ret;
    pref_set(wm, h, b"Retaliation", yes_no(ret));
    let dcam = is_checked(wm, h, C_DOUBLE_CLICK);
    h.global_data().double_click_attack_move = dcam;
    pref_set(wm, h, b"UseDoubleClickAttackMove", yes_no(dcam));
    // scroll speed
    let val = get_slider(wm, h, C_SCROLL);
    if val != -1 {
        h.global_data().keyboard_scroll_factor = val as f32 / 100.0f32;
        pref_set(wm, h, b"ScrollFactor", format!("{val}").as_bytes());
    }
    // music volume
    let val = get_slider(wm, h, C_MUSIC);
    if val != -1 {
        h.global_data().music_volume_factor = val as f32;
        pref_set(wm, h, b"MusicVolume", format!("{val}").as_bytes());
        h.set_volume(val as f32 / 100.0f32, AUDIO_AFFECT_MUSIC | AUDIO_AFFECT_SYSTEM_SETTING);
    }
    // SFX volume: one slider for 2-D and 3-D, the relative volume lowering one of them
    let val = get_slider(wm, h, C_SFX);
    if val != -1 {
        let mut sound_2d = val as f32 / 100.0f32;
        let mut sound_3d = val as f32 / 100.0f32;
        let rel = h.audio_settings().relative_2d_volume;
        // MIN(1.0f, MAX(-1.0, rel)) (in double, back to Real)
        let rel = f64::from(rel).clamp(-1.0, 1.0) as f32;
        if rel < 0.0f32 {
            sound_2d *= 1.0f32 + rel;
        } else {
            sound_3d *= 1.0f32 - rel;
        }
        h.set_volume(sound_2d, AUDIO_AFFECT_SOUND | AUDIO_AFFECT_SYSTEM_SETTING);
        h.set_volume(sound_3d, AUDIO_AFFECT_SOUND_3D | AUDIO_AFFECT_SYSTEM_SETTING);
        h.global_data().sfx_volume_factor = val as f32;
        pref_set(wm, h, b"SFXVolume", format!("{}", real_to_int(sound_2d * 100.0f32)).as_bytes());
        pref_set(wm, h, b"SFX3DVolume", format!("{}", real_to_int(sound_3d * 100.0f32)).as_bytes());
    }
    // voice volume
    let val = get_slider(wm, h, C_VOICE);
    if val != -1 {
        h.global_data().voice_volume_factor = val as f32;
        pref_set(wm, h, b"VoiceVolume", format!("{val}").as_bytes());
        h.set_volume(val as f32 / 100.0f32, AUDIO_AFFECT_SPEECH | AUDIO_AFFECT_SYSTEM_SETTING);
    }
    // gamma
    let val = get_slider(wm, h, C_GAMMA);
    if val != -1 {
        let gammaval = gamma_of(val).unwrap_or(1.0f32);
        pref_set(wm, h, b"Gamma", format!("{val}").as_bytes());
        if h.global_data().display_gamma != gammaval {
            h.global_data().display_gamma = gammaval;
            h.set_gamma(gammaval);
        }
    }
}

/// `DestroyOptionsLayout()`.
fn destroy_options_layout<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H) {
    crate::client::shell_hooks::signal_ui_interaction(h, HOOK_CLOSED);
    shell::destroy_options_layout(wm, h);
    h.options_menu().layout = None;
}

/// `WinAdvancedDisplay->winHide(hide)` (a missing window is dereferenced).
fn hide_advanced<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H, hide: bool) {
    match ctl(h, C_ADVANCED) {
        Some(w) => wm.win_hide(w, hide),
        None => wm.set_undefined("OptionsMenu: WinAdvancedDisplay->winHide on NULL"),
    }
}

/// `showAdvancedOptions()`.
pub fn show_advanced_options<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H) {
    hide_advanced(wm, h, false);
}

/// `acceptAdvancedOptions()`.
pub fn accept_advanced_options<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H) {
    hide_advanced(wm, h, true);
}

/// `cancelAdvancedOptions()`: the detail back to the LOD manager's level, the window hidden.
pub fn cancel_advanced_options<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H) {
    let level = h.static_lod_level();
    select_detail_for_level(wm, h, level);
    hide_advanced(wm, h, true);
}

/// Init's fill of one IP combo box (the LAN one, or the online one): the addresses, the preferred one selected, else
/// the first, whose text then becomes the preference.
fn fill_ip_combo<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H, c: usize, online: bool, color: i32) {
    let ips = h.ip_addresses();
    let default_ip = h.global_data().default_ip;
    let selected_ip = match h.options_menu().pref.as_mut() {
        Some(p) if online => p.get_online_ip_address(&ips, default_ip),
        Some(p) => p.get_lan_ip_address(&ips, default_ip),
        None => default_ip,
    };
    let w = ctl(h, c);
    let mut selected_index = -1;
    if let Some(w) = w {
        cb::reset(wm, h, w);
    }
    for (s, ip) in &ips {
        let text: Vec<u16> = s.iter().map(|&b| u16::from(b)).collect();
        if let Some(w) = w {
            let index = cb::add_entry(wm, h, w, &text, color);
            cb::set_item_data(wm, h, w, index, *ip);
            if selected_ip == *ip {
                selected_index = index;
            }
        }
    }
    if selected_index >= 0 {
        set_selected(wm, h, c, selected_index);
    } else {
        set_selected(wm, h, c, 0);
        if let (Some((s, _)), Some(p)) = (ips.first(), h.options_menu().pref.as_mut()) {
            if online {
                p.set_online_ip_address_str(s);
            } else {
                p.set_lan_ip_address_str(s);
            }
        }
    }
}

/// (#160) `GameLODManager::init`'s reads of `Options.ini`: the two entries (the manager turns them into levels, as
/// `getStaticGameDetail` and `getIdealStaticGameDetail` do through it) and the custom level's getters.
impl crate::client::game_lod::LodOptionPrefs for OptionPreferences {
    fn static_game_lod(&self) -> Option<Vec<u8>> {
        self.found(b"StaticGameLOD")
    }
    fn ideal_static_game_lod(&self) -> Option<Vec<u8>> {
        self.found(b"IdealStaticGameLOD")
    }
    fn custom(&mut self, g: &GlobalData) -> crate::client::game_lod::LodCustomPrefs {
        crate::client::game_lod::LodCustomPrefs {
            texture_reduction: self.get_texture_reduction(),
            shadows_3d: self.get_3d_shadows_enabled(g),
            shadows_2d: self.get_2d_shadows_enabled(g),
            building_occlusion: self.get_building_occlusion_enabled(g),
            particle_cap: self.get_particle_cap(g),
            dynamic_lod: self.get_dynamic_lod_enabled(g),
            fps_limit: self.get_fps_limit_enabled(g),
            lightmap: self.get_lightmap_enabled(g),
            cloud_shadows: self.get_cloud_shadows_enabled(g),
            smooth_water: self.get_smooth_water_enabled(g),
            heat_effects: self.get_use_heat_effects(g),
            extra_animations_disabled: self.get_extra_animations_disabled(g),
            trees: self.get_trees_enabled(g),
        }
    }
}

/// `OptionsMenuInit(layout, userData)`.
#[allow(unused_assignments)] // (upstream's `selectedXRes = 800; selectedXRes = 600;`, kept)
pub fn options_menu_init<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    h.options_menu().ignore_selected = true;
    if h.quitting() {
        return;
    }
    h.options_menu().layout = Some(layout);
    if h.options_menu().pref.is_none() {
        let p = OptionPreferences::new(wm, h);
        h.options_menu().pref = Some(p);
    }
    crate::client::shell_hooks::signal_ui_interaction(h, HOOK_OPENED);
    for (i, n) in CONTROL_NAMES.iter().enumerate() {
        let id = h.name_to_key(format!("OptionsMenu.wnd:{n}").as_bytes()) as i32;
        let w = wm.win_get_window_from_id(None, id);
        let m = h.options_menu();
        m.ids[i] = id;
        m.ctl[i] = w;
    }
    hide_advanced(wm, h, true);
    let color = game_make_color(255, 255, 255, 255);
    // the version label (`TheVersion->showFullVersion()` is false: the plain format)
    let version_id = h.name_to_key(b"OptionsMenu.wnd:LabelVersion") as i32;
    let label_version = wm.win_get_window_from_id(None, version_id);
    let v = h.registry_version();
    let fmt = h.fetch_text(b"Version:Format2");
    let version = format_wide(&fmt, &[FormatArg::Int(i64::from(v >> 16)), FormatArg::Int(i64::from(v & 0xffff))]);
    static_text_set_text(wm, h, label_version, &version);
    // the IP addresses (the online list only if its combo box exists)
    fill_ip_combo(wm, h, C_LAN_IP, false, color);
    if ctl(h, C_ONLINE_IP).is_some() {
        fill_ip_combo(wm, h, C_ONLINE_IP, true, color);
    }
    // HTTP proxy
    let proxy_id = h.name_to_key(b"OptionsMenu.wnd:TextEntryHTTPProxy") as i32;
    if let Some(entry) = wm.win_get_window_from_id(None, proxy_id) {
        let proxy: Vec<u16> = h.registry_proxy().iter().map(|&b| u16::from(b)).collect();
        te::set_text(wm, h, Some(entry), &proxy);
    }
    // firewall port override
    let port_id = h.name_to_key(b"OptionsMenu.wnd:TextEntryFirewallPortOverride") as i32;
    if let Some(entry) = wm.win_get_window_from_id(None, port_id) {
        let o = h.global_data().firewall_port_override;
        let text: Vec<u16> = if o != 0 { format!("{}", o as i32).encode_utf16().collect() } else { Vec::new() };
        te::set_text(wm, h, Some(entry), &text);
    }
    // anti-aliasing modes
    let selected_aliasing = match h.options_menu().pref.as_mut() {
        Some(p) => p.index(b"AntiAliasing"),
        None => Vec::new(),
    };
    if let Some(w) = ctl(h, C_ANTI_ALIAS) {
        cb::reset(wm, h, w);
        for i in 0..NUM_ALIASING_MODES {
            let s = h.fetch_text(format!("GUI:AntiAliasing{i}").as_bytes());
            cb::add_entry(wm, h, w, &s, color);
        }
    }
    let mut undefined = Vec::new();
    let mut val = atoi(&selected_aliasing, &mut undefined);
    // (`val < 0 || val > NUM_ALIASING_MODES`: 3 accepted, kept)
    if !(0..=NUM_ALIASING_MODES).contains(&val) {
        val = 0;
        h.global_data().anti_alias_box_value = 0;
    }
    set_selected(wm, h, C_ANTI_ALIAS, val);
    // the saved display mode
    let selected_resolution = match h.options_menu().pref.as_mut() {
        Some(p) => p.index(b"Resolution"),
        None => Vec::new(),
    };
    let (mut sel_x, mut sel_y) = (800, 600);
    if !selected_resolution.is_empty() {
        match scan_two_ints(&selected_resolution, &mut undefined) {
            Some((x, y)) => (sel_x, sel_y) = (x, y),
            None => (sel_x, sel_y) = (800, 600),
        }
    }
    if !undefined.is_empty() {
        wm.set_undefined("OptionsMenuInit: atoi / sscanf of a preference outside Int's range");
    }
    let modes = h.display_modes();
    if let Some(w) = ctl(h, C_RESOLUTION) {
        cb::reset(wm, h, w);
    }
    let mut selected_res_index = -1;
    let mut default_res_index = 0;
    for (i, &(x, y, _)) in modes.iter().enumerate() {
        let s: Vec<u16> = format!("{x} x {y}").encode_utf16().collect();
        if let Some(w) = ctl(h, C_RESOLUTION) {
            cb::add_entry(wm, h, w, &s, color);
        }
        if x == 800 && y == 600 {
            default_res_index = i as i32;
        }
        if x == sel_x && y == sel_y {
            selected_res_index = i as i32;
        }
    }
    if selected_res_index == -1 {
        // (upstream's typo, kept: both assignments go to X)
        sel_x = 800;
        sel_x = 600;
        selected_res_index = default_res_index;
    }
    h.global_data().x_resolution = sel_x;
    h.global_data().y_resolution = sel_y;
    set_selected(wm, h, C_RESOLUTION, selected_res_index);
    // the display detail
    if let Some(w) = ctl(h, C_DETAIL) {
        cb::reset(wm, h, w);
        for l in [&b"GUI:High"[..], b"GUI:Medium", b"GUI:Low", b"GUI:Custom"] {
            let s = h.fetch_text(l);
            cb::add_entry(wm, h, w, &s, color);
        }
    }
    if h.static_lod_level() == STATIC_GAME_LOD_UNKNOWN {
        let l = h.find_static_lod_level();
        h.set_static_lod_level(l);
    }
    let level = h.static_lod_level();
    select_detail_for_level(wm, h, level);
    let txt_fact = h.current_texture_reduction();
    set_slider(wm, h, C_TEXTURE, 2 - txt_fact);
    let g = h.global_data().clone();
    set_checked(wm, h, C_3D_SHADOWS, g.use_shadow_volumes);
    set_checked(wm, h, C_2D_SHADOWS, g.use_shadow_decals);
    set_checked(wm, h, C_CLOUD, g.use_cloud_map);
    set_checked(wm, h, C_GROUND, g.use_light_map);
    set_checked(wm, h, C_WATER, g.show_soft_water_edge);
    set_checked(wm, h, C_EXTRA_ANIM, !g.use_draw_module_lod);
    set_checked(wm, h, C_NO_DYNAMIC_LOD, !g.enable_dynamic_lod);
    set_checked(wm, h, C_HEAT, g.use_heat_effects);
    set_checked(wm, h, C_UNLOCK_FPS, !g.use_fps_limit);
    set_checked(wm, h, C_BUILDING, g.enable_behind_building_markers);
    set_checked(wm, h, C_PROPS, g.use_trees);
    set_slider(wm, h, C_PARTICLE, g.max_particle_count);
    // the language filter
    let lang = h.options_menu().pref.as_mut().map(|p| p.index(b"LanguageFilter")).unwrap_or_default();
    let on = lang == b"true";
    set_checked(wm, h, C_LANGUAGE, on);
    h.global_data().language_filter_pref = on;
    // the replay camera
    let (save, use_) = h.options_menu().pref.as_ref().map_or((true, true), |p| (p.save_camera_in_replays(), p.use_camera_in_replays()));
    set_checked(wm, h, C_SAVE_CAMERA, save);
    h.global_data().save_camera_in_replay = save;
    set_checked(wm, h, C_USE_CAMERA, use_);
    h.global_data().use_camera_in_replay = use_;
    // the scroll anchors
    let test = h.options_menu().pref.as_mut().map(|p| p.index(b"DrawScrollAnchor")).unwrap_or_default();
    let on = test == b"Yes" || (test.is_empty() && *h.draw_rmb_scroll_anchor());
    set_checked(wm, h, C_DRAW_ANCHOR, on);
    *h.draw_rmb_scroll_anchor() = on;
    let test = h.options_menu().pref.as_mut().map(|p| p.index(b"MoveScrollAnchor")).unwrap_or_default();
    let on = test == b"Yes" || (test.is_empty() && *h.move_rmb_scroll_anchor());
    set_checked(wm, h, C_MOVE_ANCHOR, on);
    *h.move_rmb_scroll_anchor() = on;
    // the mouse mode
    let g = h.global_data().clone();
    set_checked(wm, h, C_ALT_MOUSE, g.use_alternate_mouse);
    set_checked(wm, h, C_RETALIATION, g.client_retaliation_mode_enabled);
    set_checked(wm, h, C_DOUBLE_CLICK, g.double_click_attack_move);
    set_slider(wm, h, C_SCROLL, (g.keyboard_scroll_factor * 100.0f32) as i32);
    set_checked(wm, h, C_SEND_DELAY, g.firewall_send_delay);
    // the volumes and the gamma
    let a = h.audio_settings();
    let (music, sfx, sfx3d, voice, gamma) = match h.options_menu().pref.as_mut() {
        Some(p) => (p.get_music_volume(&a), p.get_sound_volume(&a), p.get_3d_sound_volume(&a), p.get_speech_volume(&a), p.get_gamma_value()),
        None => (0.0, 0.0, 0.0, 0.0, 50.0),
    };
    set_slider(wm, h, C_MUSIC, real_to_int(music));
    set_slider(wm, h, C_SFX, real_to_int(if sfx > sfx3d { sfx } else { sfx3d }));
    set_slider(wm, h, C_VOICE, real_to_int(voice));
    set_slider(wm, h, C_GAMMA, real_to_int(gamma));
    if let Some(mut p) = h.options_menu().pref.take() {
        report_undefined(wm, &mut p.prefs);
        h.options_menu().pref = Some(p);
    }
    // show the menu, focus and make modal its parent
    wm.layout_hide(layout, false);
    let parent_id = h.name_to_key(b"OptionsMenu.wnd:OptionsMenuParent") as i32;
    let parent = wm.win_get_window_from_id(None, parent_id);
    wm.win_set_focus(h, parent);
    let env = h.env();
    if env.in_game && h.game_mode() != game_mode::SHELL {
        for c in [C_LAN_IP, C_SEND_DELAY, C_FIREWALL_REFRESH] {
            match ctl(h, c) {
                Some(w) => wm.win_enable(w, false),
                None => wm.set_undefined("OptionsMenuInit: winEnable on a missing control"),
            }
        }
        for c in [C_ONLINE_IP, C_DETAIL, C_RESOLUTION] {
            if let Some(w) = ctl(h, c) {
                wm.win_enable(w, false);
            }
        }
    }
    wm.win_set_modal(parent);
    h.options_menu().ignore_selected = false;
}

/// `OptionsMenuShutdown(layout, userData)`: nothing (its body was moved into the buttons).
pub fn options_menu_shutdown<H: OptionsHost>(_wm: &mut GameWindowManager, _h: &mut H, _layout: usize) {}

/// `OptionsMenuUpdate(layout, userData)`: nothing.
pub fn options_menu_update<H: OptionsHost>(_wm: &mut GameWindowManager, _h: &mut H, _layout: usize) {}

/// `OptionsMenuInput(window, msg, mData1, mData2)`: Escape (handled whatever its state) presses *Back* on its release,
/// the button looked up under `window`.
pub fn options_menu_input<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg != gwm::CHAR || (d1 & 0xFF) as u8 != key::ESC {
        return MsgHandled::Ignored;
    }
    if (d2 & 0xFF) & key::STATE_UP != 0 {
        let id = h.name_to_key(b"OptionsMenu.wnd:ButtonBack") as i32;
        let button = wm.win_get_window_from_id(Some(window), id);
        wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(button), id as u32);
    }
    MsgHandled::Handled
}

/// A check box that stores at once: `(*pref)[key]` = `on` / `off` and the setter.
fn check_now<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H, control: WinId, key: &[u8], on: &[u8], off: &[u8], set: impl Fn(&mut H, bool)) {
    let checked = check_box_is_checked(wm, control);
    set(h, checked);
    pref_set(wm, h, key, if checked { on } else { off });
}

/// `OptionsMenuSystem(window, msg, mData1, mData2)`.
pub fn options_menu_system<H: OptionsHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE => {
            let mut k = |n: &str| h.name_to_key(format!("OptionsMenu.wnd:{n}").as_bytes()) as i32;
            let (back, defaults, accept, keyboard) = (k("ButtonBack"), k("ButtonDefaults"), k("ButtonAccept"), k("ButtonKeyboardOptions"));
            let m = h.options_menu();
            m.button_back = back;
            m.button_defaults = defaults;
            m.button_accept = accept;
            m.button_keyboard_options_menu = keyboard;
        }
        gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
        }
        ggm::GCM_SELECTED => {
            if h.options_menu().ignore_selected {
                return MsgHandled::Handled;
            }
            let Some(control) = data_win(d1) else {
                wm.set_undefined("OptionsMenuSystem: GCM_SELECTED with no control");
                return MsgHandled::Handled;
            };
            if wm.tree.windows[control].id == h.options_menu().ids[C_DETAIL] {
                let mut index = None;
                get_selected(wm, h, C_DETAIL, &mut index);
                if index == Some(CUSTOMDETAIL) {
                    show_advanced_options(wm, h);
                }
            }
        }
        ggm::GBM_SELECTED => {
            if h.options_menu().ignore_selected {
                return MsgHandled::Handled;
            }
            let Some(control) = data_win(d1) else {
                wm.set_undefined("OptionsMenuSystem: GBM_SELECTED with no control (control->winGetWindowId)");
                return MsgHandled::Handled;
            };
            let id = wm.tree.windows[control].id;
            let m = h.options_menu().clone();
            if id == m.button_back {
                let m = h.options_menu();
                m.pref = None;
                m.ctl[C_LAN_IP] = None;
                m.ctl[C_ONLINE_IP] = None;
                destroy_options_layout(wm, h);
            } else if id == m.button_accept {
                save_options(wm, h);
                if let Some(p) = h.options_menu().pref.take() {
                    p.write(&mut HostUserData(h));
                }
                let m = h.options_menu();
                m.ctl[C_LAN_IP] = None;
                m.ctl[C_ONLINE_IP] = None;
                let env = h.env();
                if !env.in_game || env.in_shell_game {
                    h.options_destroy_quit_menu(wm);
                }
                destroy_options_layout(wm, h);
                if h.options_menu().disp_changed {
                    h.do_resolution_dialog(wm);
                }
            } else if id == m.button_defaults {
                set_defaults(wm, h);
            } else if id == m.ids[C_ADV_ACCEPT] {
                accept_advanced_options(wm, h);
            } else if id == m.ids[C_ADV_CANCEL] {
                cancel_advanced_options(wm, h);
            } else if id == m.button_keyboard_options_menu {
                shell::push(wm, h, b"Menus/KeyboardOptionsMenu.wnd", false);
            } else if id == m.ids[C_DRAW_ANCHOR] {
                check_now(wm, h, control, b"DrawScrollAnchor", b"Yes", b"No", |h, b| *h.draw_rmb_scroll_anchor() = b);
            } else if id == m.ids[C_MOVE_ANCHOR] {
                check_now(wm, h, control, b"MoveScrollAnchor", b"Yes", b"No", |h, b| *h.move_rmb_scroll_anchor() = b);
            } else if id == m.ids[C_SAVE_CAMERA] {
                check_now(wm, h, control, b"SaveCameraInReplays", b"yes", b"no", |h, b| h.global_data().save_camera_in_replay = b);
            } else if id == m.ids[C_USE_CAMERA] {
                check_now(wm, h, control, b"UseCameraInReplays", b"yes", b"no", |h, b| h.global_data().use_camera_in_replay = b);
            } else if id == m.ids[C_FIREWALL_REFRESH] {
                h.global_data().firewall_behavior = FIREWALL_TYPE_UNKNOWN;
                // itoa(m_firewallBehavior, num, 10)
                let num = format!("{}", h.global_data().firewall_behavior as i32);
                pref_set(wm, h, b"FirewallBehavior", num.as_bytes());
            }
            let _ = d2;
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::save_load_menu::tests::{booted, click, settle, H};
    use crate::client::shell::{self as sh, ShellRequest};
    use crate::client::user_preferences::PreferenceMap;
    use crate::client::window_manager::WIN_STATE_SELECTED;
    use crate::client::window_script::win_status;

    const F: &str = "OptionsMenu.wnd";

    fn map(pairs: &[(&str, &str)]) -> PreferenceMap {
        pairs.iter().map(|(k, v)| (k.as_bytes().to_vec(), v.as_bytes().to_vec())).collect()
    }

    /// The last window `OptionsMenu.wnd:<n>` (a screen opened again makes new windows; freed ones stay in the tree).
    fn last(wm: &GameWindowManager, n: &str) -> WinId {
        let full = format!("{F}:{n}");
        (0..wm.tree.windows.len()).rev().find(|&w| wm.tree.windows[w].name() == full.as_bytes()).unwrap_or_else(|| panic!("{n}"))
    }

    fn checked(wm: &GameWindowManager, n: &str) -> bool {
        wm.tree.windows[last(wm, n)].inst.state & WIN_STATE_SELECTED != 0
    }

    fn slider(wm: &GameWindowManager, n: &str) -> i32 {
        slider_get_position(wm, last(wm, n))
    }

    /// The options layout opened as the main menu's *Options* opens it (`getOptionsLayout(TRUE)`, `runInit`).
    fn open(wm: &mut GameWindowManager, h: &mut H) -> usize {
        let l = sh::get_options_layout(wm, h, true).expect("the options layout loads");
        sh::run_layout(wm, h, l, 0, None);
        wm.layout_hide(l, false);
        l
    }

    /// #88: the options screen over `Options.ini`, predicted from OptionsMenu.cpp and GlobalData.cpp before the run:
    ///
    /// - the start (`parseGameDataDefinition`'s tail, [`apply_option_preferences`]): scroll factor 0.7, gamma 75 is
    ///   1.5, resolution 1024 x 768;
    /// - Init: the scroll slider 70 (from the global data), music 40, SFX 60 (the larger of 30 and 60), voice 55,
    ///   gamma 75; the language filter checked, the replay camera's *save* unchecked (global false), *use* checked;
    ///   the draw anchor checked (in-game UI true), the move anchor not (no entry: the UI's false, and an empty entry
    ///   inserted); the anti-aliasing entry 3 kept (the `>` quirk); with no display modes the saved one is missing, so
    ///   the global resolution becomes 600 x 768 (the `selectedXRes` typo); empty `IPAddress` / `GameSpyIPAddress`
    ///   inserted; the advanced window hidden; the parent modal; the version label `T:Version:Format2`; nothing
    ///   undefined;
    /// - *Custom* detail shows the advanced window; its *Back* hides it again;
    /// - the music slider at 80, then *Accept*: `MusicVolume = 80`, `SFXVolume = SFX3DVolume = 60` (no relative
    ///   volume), `VoiceVolume = 55`, `Gamma = 75`, `ScrollFactor = 70`, `StaticGameLOD = Custom`, the three volumes
    ///   asked of the audio (0.8 music, 0.6 sound and 3-D sound, 0.55 speech), no gamma change (1.5 already); the
    ///   file written once, in CR LF lines; the layout destroyed, the closed hook signalled, the pref object gone;
    /// - opened again: Escape's release closes it without writing; *Defaults* sets the scroll slider to 50 (the
    ///   default factor), the gamma to 50 (the slider's middle); the *use camera* box stores `no` at once; *Firewall
    ///   Refresh* stores `FirewallBehavior = 0`; *Back* writes nothing.
    ///
    /// **Result (2026-11-10):** as predicted for the screen; three misses in the test's own steps: it expected two
    /// writes in the host's queue, but its own setup put `Options.ini` into the folder directly (only *Accept*'s is
    /// queued); and twice it found the closed screen's freed windows first by name (Escape sent to the old parent;
    /// the sliders read -1 from the old ones): the test now takes the last window of a name.
    #[test]
    fn the_options_menu_reads_and_writes_options_ini() {
        let (mut wm, mut h) = booted();
        h.set_pref_file(
            b"Options.ini",
            &map(&[
                ("ScrollFactor", "70"),
                ("MusicVolume", "40"),
                ("SFXVolume", "30"),
                ("SFX3DVolume", "60"),
                ("VoiceVolume", "55"),
                ("Gamma", "75"),
                ("AntiAliasing", "3"),
                ("Resolution", "1024 768"),
                ("LanguageFilter", "true"),
                ("SaveCameraInReplays", "no"),
                ("DrawScrollAnchor", "Yes"),
            ]),
        );
        let mut p = OptionPreferences::load_from(&mut h.user_data);
        apply_option_preferences(&mut h.global_data, &mut p, &[]);
        assert_eq!((h.global_data.keyboard_scroll_factor, h.global_data.display_gamma), (0.7f32, 1.5f32));
        assert_eq!((h.global_data.x_resolution, h.global_data.y_resolution), (1024, 768));
        assert!(!h.global_data.save_camera_in_replay && h.global_data.use_camera_in_replay);
        let l = open(&mut wm, &mut h);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        assert_eq!((slider(&wm, "SliderScrollSpeed"), slider(&wm, "SliderMusicVolume"), slider(&wm, "SliderSFXVolume")), (70, 40, 60));
        assert_eq!((slider(&wm, "SliderVoiceVolume"), slider(&wm, "SliderGamma")), (55, 75));
        assert!(checked(&wm, "CheckLanguageFilter") && !checked(&wm, "CheckBoxSaveCamera") && checked(&wm, "CheckBoxUseCamera"));
        assert!(checked(&wm, "CheckBoxDrawAnchor") && !checked(&wm, "CheckBoxMoveAnchor"));
        assert!(h.rmb_scroll_anchor.0 && !h.rmb_scroll_anchor.1);
        assert!(h.global_data.language_filter_pref);
        let pref = h.options.pref.clone().expect("pref");
        assert_eq!(pref.map.get(&b"AntiAliasing"[..]).map(|v| v.as_slice()), Some(&b"3"[..]));
        for k in [&b"MoveScrollAnchor"[..], b"IPAddress", b"GameSpyIPAddress"] {
            assert_eq!(pref.map.get(k).map(|v| v.as_slice()), Some(&b""[..]), "{}", String::from_utf8_lossy(k));
        }
        assert_eq!((h.global_data.x_resolution, h.global_data.y_resolution), (600, 768));
        assert!(wm.tree.windows[last(&wm, "WinAdvancedDisplayOptions")].status & win_status::HIDDEN != 0);
        let parent = last(&wm, "OptionsMenuParent");
        assert_eq!(wm.modal.first().copied(), Some(parent));
        let label = crate::client::gadget::static_text_get_text(&wm, last(&wm, "LabelVersion"));
        assert_eq!(String::from_utf16_lossy(&label), "T:Version:Format2");
        assert!(h.ui_interacts.contains(&"ShellOptionsOpened"));
        // Custom detail: the advanced window; its Back
        let detail = last(&wm, "ComboBoxDetail");
        cb::set_selected_pos(&mut wm, &mut h, detail, CUSTOMDETAIL, false);
        assert!(wm.tree.windows[last(&wm, "WinAdvancedDisplayOptions")].status & win_status::HIDDEN == 0);
        let adv_back = last(&wm, "ButtonAdvanceBack");
        click(&mut wm, &mut h, adv_back);
        assert!(wm.tree.windows[last(&wm, "WinAdvancedDisplayOptions")].status & win_status::HIDDEN != 0);
        // the music slider, then Accept
        let music = last(&wm, "SliderMusicVolume");
        slider_set_position(&mut wm, &mut h, music, 80);
        h.requests.clear();
        let accept = last(&wm, "ButtonAccept");
        click(&mut wm, &mut h, accept);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        let written = h.pref_file(b"Options.ini").expect("written");
        let get = |k: &str| written.get(k.as_bytes()).map(|v| String::from_utf8_lossy(v).into_owned());
        assert_eq!((get("MusicVolume"), get("SFXVolume"), get("SFX3DVolume")), (Some("80".into()), Some("60".into()), Some("60".into())));
        assert_eq!((get("VoiceVolume"), get("Gamma"), get("ScrollFactor")), (Some("55".into()), Some("75".into()), Some("70".into())));
        // ~~`StaticGameLOD = Custom`~~ **Corrected 2026-11-12 (#160):** with the translated LOD manager the init finds and
        // sets the level of this (unknown) machine, low; *Custom*'s Back puts the combo box back on it, so Accept's
        // `setStaticLODLevel(LOW)` changes nothing and writes no `StaticGameLOD` (upstream's behaviour; the stub before
        // reported every level as a change). The manager's own save of it (`findStaticLODLevel`) went to the file before
        // the screen's preferences, read at its init, overwrote it.
        assert_eq!(get("StaticGameLOD"), None);
        let sys = AUDIO_AFFECT_SYSTEM_SETTING;
        assert_eq!(
            h.requests,
            [
                ShellRequest::SetVolume { volume: 0.8, affect: AUDIO_AFFECT_MUSIC | sys },
                ShellRequest::SetVolume { volume: 0.6, affect: AUDIO_AFFECT_SOUND | sys },
                ShellRequest::SetVolume { volume: 0.6, affect: AUDIO_AFFECT_SOUND_3D | sys },
                ShellRequest::SetVolume { volume: 0.55, affect: AUDIO_AFFECT_SPEECH | sys },
            ]
        );
        assert!(!h.untranslated.contains(&b"Display::setGamma".to_vec()));
        // ~~1, Accept's~~ **Corrected 2026-11-12 (#160):** 2: the init's `findStaticLODLevel` save, then Accept's (the
        // test's own file was put in the folder directly)
        assert_eq!(h.user_data_writes.len(), 2, "the LOD manager's save and Accept's");
        assert!(h.user_data_writes[0].1.windows(2).any(|w| w == b"\r\n"));
        assert!(wm.layouts[l].deleted && h.shell.options_layout.is_none() && h.options.pref.is_none());
        assert!(h.ui_interacts.contains(&"ShellOptionsClosed"));
        // opened again: Escape (the closed screen's windows freed first: `processDestroyList`)
        settle(&mut wm, &mut h);
        let l = open(&mut wm, &mut h);
        let parent = wm.layouts[l].windows[0];
        options_menu_input(&mut wm, &mut h, parent, gwm::CHAR, u32::from(key::ESC), key::STATE_UP);
        // (#160: the ideal level is known now: no second LOD save)
        assert!(wm.layouts[l].deleted && h.user_data_writes.len() == 2);
        // and again: Defaults, the camera box, Firewall Refresh, Back
        settle(&mut wm, &mut h);
        let _ = open(&mut wm, &mut h);
        let defaults = last(&wm, "ButtonDefaults");
        click(&mut wm, &mut h, defaults);
        assert_eq!((slider(&wm, "SliderScrollSpeed"), slider(&wm, "SliderGamma")), (50, 50));
        let cam = last(&wm, "CheckBoxUseCamera");
        click(&mut wm, &mut h, cam);
        assert!(!h.global_data.use_camera_in_replay);
        assert_eq!(h.options.pref.as_ref().and_then(|p| p.find(b"UseCameraInReplays")), Some(&b"no"[..]));
        let fw = last(&wm, "ButtonFirewallRefresh");
        click(&mut wm, &mut h, fw);
        assert_eq!(h.options.pref.as_ref().and_then(|p| p.find(b"FirewallBehavior")), Some(&b"0"[..]));
        let back = last(&wm, "ButtonBack");
        click(&mut wm, &mut h, back);
        settle(&mut wm, &mut h);
        // (#160: the two writes above, the LOD manager's and Accept's: ~~1~~)
        assert_eq!(h.user_data_writes.len(), 2, "Back writes nothing");
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        assert_eq!(options_menu_input(&mut wm, &mut h, parent, gwm::CHAR, 0, 0), MsgHandled::Ignored);
    }

    /// #88: `OptionPreferences`' getters, predicted from OptionsMenu.cpp before the run: the global data's (or the
    /// audio settings') value without the entry; `yes` in any case is true, anything else false; the clamps
    /// (scroll 0 .. 100, campaign easy .. hard, particles at least 100, texture at most 2, port 0 outside 0 .. 65535,
    /// firewall behaviour at least 0, the allocation delta truncated to 16 bits); the extra animations inverted; the
    /// resolution only from two numbers; the IP addresses by their text without case, inserting the empty entry.
    ///
    /// **Result (2026-11-10):** as predicted.
    #[test]
    fn option_preferences_read_as_upstream() {
        let g = GlobalData::new();
        let mut p = OptionPreferences::default();
        assert_eq!(p.get_campaign_difficulty(1), 1);
        assert_eq!(p.get_scroll_factor(&g), g.keyboard_default_scroll_factor);
        assert_eq!((p.get_texture_reduction(), p.get_gamma_value(), p.get_particle_cap(&g)), (-1, 50.0, g.max_particle_count));
        assert!(p.uses_system_map_dir() && p.save_camera_in_replays() && p.use_camera_in_replays() && !p.get_firewall_need_to_refresh());
        assert_eq!(p.get_resolution(&g), (g.x_resolution, g.y_resolution));
        assert_eq!(p.get_extra_animations_disabled(&g), g.use_draw_module_lod);
        let ips = [(b"10.0.0.2".to_vec(), 0x0a00_0002), (b"192.168.1.5".to_vec(), 0xc0a8_0105)];
        assert_eq!(p.get_lan_ip_address(&ips, 7), 7);
        assert_eq!(p.map.get(&b"IPAddress"[..]).map(|v| v.len()), Some(0), "operator[] inserts");
        for (k, v) in [
            ("CampaignDifficulty", "9"),
            ("ScrollFactor", "250"),
            ("UseAlternateMouse", "YES"),
            ("Retaliation", "y"),
            ("MaxParticleCount", "20"),
            ("TextureReduction", "-4"),
            ("Gamma", "33.9"),
            ("FirewallPortOverride", "70000"),
            ("FirewallBehavior", "-3"),
            ("FirewallPortAllocationDelta", "70000"),
            ("FirewallNeedToRefresh", "true"),
            ("ExtraAnimations", "yes"),
            ("Resolution", " 1280\t1024 x"),
            ("GameSpyIPAddress", "192.168.1.5"),
            ("SFXVolume", "-5"),
            ("MusicVolume", "12.5"),
        ] {
            p.set_ascii_string(k.as_bytes(), v.as_bytes());
        }
        assert_eq!(p.get_campaign_difficulty(1), DIFFICULTY_HARD);
        assert_eq!(p.get_scroll_factor(&g), 1.0);
        assert!(p.get_alternate_mouse_mode_enabled(&g) && !p.get_retaliation_mode_enabled(&g));
        assert_eq!((p.get_particle_cap(&g), p.get_texture_reduction(), p.get_gamma_value()), (100, -4, 33.0));
        assert_eq!((p.get_firewall_port_override(&g), p.get_firewall_behavior(&g), p.get_firewall_port_allocation_delta(&g)), (0, 0, 4464));
        assert!(p.get_firewall_need_to_refresh() && !p.get_extra_animations_disabled(&g));
        assert_eq!(p.get_resolution(&g), (1280, 1024));
        assert_eq!(p.get_online_ip_address(&ips, 7), 0xc0a8_0105);
        let a = AudioSettings { default_music_volume: 0.5, ..AudioSettings::default() };
        assert_eq!((p.get_sound_volume(&a), p.get_music_volume(&a), p.get_speech_volume(&a)), (0.0, 12.5, 0.0));
        p.set_lan_ip_address(0xc0a8_0105);
        assert_eq!(p.find(b"IPAddress"), Some(&b"192.168.1.5"[..]));
        p.set_ascii_string(b"Resolution", b"1280");
        assert_eq!(p.get_resolution(&g), (g.x_resolution, g.y_resolution));
        assert!(p.undefined.is_empty());
        assert_eq!((gamma_of(0), gamma_of(25), gamma_of(50), gamma_of(100)), (Some(0.6), Some(0.8), None, Some(2.0)));
    }

    /// #93: the preference files live across a restart: a skirmish's `Skirmish.ini` and the campaign difficulty's
    /// `Options.ini` written by one shell and handed (as the client hands its folder) to a new one read the same.
    /// Predicted: the second shell's options screen shows the music volume the first one saved.
    ///
    /// **Result (2026-11-10):** as predicted.
    #[test]
    fn preferences_persist_across_a_restart() {
        let (mut wm, mut h) = booted();
        let _ = open(&mut wm, &mut h);
        let music = last(&wm, "SliderMusicVolume");
        slider_set_position(&mut wm, &mut h, music, 33);
        let accept = last(&wm, "ButtonAccept");
        click(&mut wm, &mut h, accept);
        let folder: Vec<(Vec<u8>, Vec<u8>)> = std::mem::take(&mut h.user_data_writes);
        // ~~one write~~ **Corrected 2026-11-12 (#160):** two writes of Options.ini: `findStaticLODLevel`'s (the options
        // screen's init finds this machine's level, `IdealStaticGameLOD` and `StaticGameLOD` saved) and Accept's
        assert_eq!(folder.len(), 2);
        assert!(folder.iter().all(|(p, _)| p.as_slice() == b"Options.ini"));
        let (mut wm2, mut h2) = booted();
        for (path, bytes) in folder {
            h2.user_data.files.insert(path, bytes);
        }
        let _ = open(&mut wm2, &mut h2);
        assert_eq!(slider(&wm2, "SliderMusicVolume"), 33);
    }
}
