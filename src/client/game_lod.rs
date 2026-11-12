// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/GameLOD.cpp (the whole file)
//   GeneralsMD/Code/GameEngine/Include/Common/GameLOD.h (the whole file)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp (W3DDisplay::updateAverageFPS, and
//     W3DDisplay::draw's and W3DDisplay::init's calls of the LOD manager)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/OptionsMenu.cpp
//     (OptionPreferences::getStaticGameDetail, getIdealStaticGameDetail: the LOD manager's reads of Options.ini)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The game's level of detail (GitHub #160, a part of #100): `TheGameLODManager`, the five INI blocks it reads
//! (`StaticGameLOD`, `DynamicGameLOD`, `LODPreset`, `BenchProfile`, `ReallyLowMHz`), the static detail level (low,
//! medium, high, custom) and what applying it writes into the global data, and the dynamic level the frame rate picks
//! (which particles and debris are skipped, how fast slow deaths run).
//!
//! # As upstream reads it
//!
//! `GameLODManager::init` loads `Data\INI\GameLOD.ini` (the four static levels' settings and the four dynamic levels')
//! and `Data\INI\GameLODPresets.ini` (the hardware each static level is recommended for, and the benchmark profiles of
//! known processors), copies the custom level from the global data, reads the player's level from `Options.ini`, asks
//! the device for the processor, its speed and the memory (`testMinimumRequirements`), runs the benchmark when the
//! processor is unknown, and applies the player's level. `findStaticLODLevel` matches the hardware against the presets
//! (the best level whose processor, speed, video chip and memory the machine reaches) and saves the result in
//! `Options.ini`. `W3DDisplay::draw` picks the dynamic level from the average frame rate every frame.
//!
//! # Where this differs from upstream
//!
//! - **The hardware is the host's** ([`GameLodHost`]): `testMinimumRequirements` (`CPUDetectClass`,
//!   `W3DShaderManager::getChipset`, `RunBenchmark`) belongs to the device, which this translation does not have. The
//!   client's host ([`UnknownHardware`]) answers *unknown* and records each call: the processor `XX` and the video chip
//!   `DC_UNKNOWN`, upstream's own values for hardware it does not recognise (and upstream's own paths for them follow:
//!   the benchmark for an unknown processor, the TNT2 presumed for an unknown chip); the speed, the memory and the
//!   benchmark are left unwritten (their constructor's 0), as no value is faked. With the memory unknown,
//!   `m_memPassed` stays false: upstream's path for less than 256 MB (low-resolution textures, no trees, no shell
//!   map).
//! - **The global data** written is the caller's (the client's copy, `ShellGui::global_data`), never the logic's: the
//!   client never changes the simulation. Upstream's logic reads the manager in three places, which stay gaps (#160):
//!   `SlowDeathBehavior` (`getSlowDeathScale`, here `cuts.globals.slow_death_scale`), `ObjectCreationList`'s debris
//!   (`isDebrisSkipped`) and `GameLogic::startNewGame`'s fluff-to-prop (`getStaticLODLevel`).
//! - The game client's and the terrain's calls (`adjustLOD`, `releaseShadows`, `allocateShadows`, `setShoreLineDetail`,
//!   `setTerrainTracksDetail`) and the `Benchmark.txt` dump are the host's.
//! - Each undefined step of upstream's is recorded in [`GameLodManager::undefined`] and not acted on: a `LODPreset`
//!   of the level `Custom` (`m_numLevelPresets[3]`, past the array), a level outside its table, the dynamic priorities
//!   read before a dynamic level was ever applied (the constructor leaves them uninitialised), a frame rate that is not
//!   an `Int`. A field the two field tables do not hold is [`Undefined::UnterminatedFieldTable`] (the tables have no
//!   terminator, so `findFieldParse` reads past them).
//!
//! # Verified against the compiled upstream (#160)
//!
//! `tests/differential_game_lod.rs`: GameLOD.cpp cut verbatim on the real INI reader, 20,000 random cases (the five
//! blocks, good and malformed; the machine known and unknown; every operation; W3DDisplay's frame rate and levels):
//! 0 mismatches; 16 of 16 planted bugs (9541-9556) caught.

use crate::common::global_data::GlobalData;
use crate::common::ini::{BlockParse, FieldParse, Ini, IniError, IniResult, Undefined, UserData};
use crate::client::particle_sys::{priority, PARTICLE_PRIORITY_NAMES};
use crate::logic::modules::plant;

/// `MAX_LOD_PRESETS_PER_LEVEL`.
pub const MAX_LOD_PRESETS_PER_LEVEL: usize = 32;
/// `MAX_BENCH_PROFILES`.
pub const MAX_BENCH_PROFILES: usize = 16;
/// `PROFILE_ERROR_LIMIT`: the fraction of a profile's result needed to match it.
pub const PROFILE_ERROR_LIMIT: f32 = 0.94;

/// `enum StaticGameLODLevel`.
pub const STATIC_GAME_LOD_UNKNOWN: i32 = -1;
pub const STATIC_GAME_LOD_LOW: i32 = 0;
pub const STATIC_GAME_LOD_MEDIUM: i32 = 1;
pub const STATIC_GAME_LOD_HIGH: i32 = 2;
pub const STATIC_GAME_LOD_CUSTOM: i32 = 3;
pub const STATIC_GAME_LOD_COUNT: i32 = 4;

/// `enum DynamicGameLODLevel`.
pub const DYNAMIC_GAME_LOD_UNKNOWN: i32 = -1;
pub const DYNAMIC_GAME_LOD_LOW: i32 = 0;
pub const DYNAMIC_GAME_LOD_MEDIUM: i32 = 1;
pub const DYNAMIC_GAME_LOD_HIGH: i32 = 2;
pub const DYNAMIC_GAME_LOD_VERY_HIGH: i32 = 3;
pub const DYNAMIC_GAME_LOD_COUNT: i32 = 4;

/// `enum CpuType`.
pub mod cpu {
    /// `XX`: unknown.
    pub const XX: i32 = 0;
    pub const P3: i32 = 1;
    pub const P4: i32 = 2;
    pub const K7: i32 = 3;
}

/// `enum ChipsetType`.
pub mod chipset {
    pub const DC_UNKNOWN: i32 = 0;
    pub const DC_VOODOO2: i32 = 1;
    pub const DC_VOODOO3: i32 = 2;
    pub const DC_VOODOO4: i32 = 3;
    pub const DC_VOODOO5: i32 = 4;
    pub const DC_TNT: i32 = 5;
    pub const DC_TNT2: i32 = 6;
    pub const DC_GEFORCE2: i32 = 7;
    pub const DC_RADEON: i32 = 8;
    pub const DC_GENERIC_PIXEL_SHADER_1_1: i32 = 9;
    pub const DC_GEFORCE3: i32 = 10;
    pub const DC_GEFORCE4: i32 = 11;
    pub const DC_GENERIC_PIXEL_SHADER_1_4: i32 = 12;
    pub const DC_RADEON_8500: i32 = 13;
    pub const DC_GENERIC_PIXEL_SHADER_2_0: i32 = 14;
    pub const DC_RADEON_9700: i32 = 15;
    pub const DC_MAX: i32 = 16;
}

/// `StaticGameLODNames`.
pub const STATIC_GAME_LOD_NAMES: [&str; 4] = ["Low", "Medium", "High", "Custom"];
/// `DynamicGameLODNames`.
pub const DYNAMIC_GAME_LOD_NAMES: [&str; 4] = ["Low", "Medium", "High", "VeryHigh"];
/// `CPUNames`.
pub const CPU_NAMES: [&str; 4] = ["XX", "P3", "P4", "K7"];
/// `VideoNames`.
pub const VIDEO_NAMES: [&str; 16] = ["XX", "V2", "V3", "V4", "V5", "TNT", "TNT2", "GF2", "R100", "PS11", "GF3", "GF4", "PS14", "R200", "PS20", "R300"];

/// `struct StaticGameLODInfo`.
#[derive(Clone, Debug, PartialEq)]
pub struct StaticGameLodInfo {
    pub min_fps: i32,
    pub min_processor_fps: i32,
    pub sample_count_2d: i32,
    pub sample_count_3d: i32,
    pub stream_count: i32,
    pub max_particle_count: i32,
    pub use_shadow_volumes: bool,
    pub use_shadow_decals: bool,
    pub use_cloud_map: bool,
    pub use_light_map: bool,
    pub show_soft_water_edge: bool,
    pub max_tank_track_edges: i32,
    pub max_tank_track_opaque_edges: i32,
    pub max_tank_track_fade_delay: i32,
    pub use_buildup_scaffolds: bool,
    pub use_tree_sway: bool,
    pub use_emissive_night_materials: bool,
    pub use_heat_effects: bool,
    pub texture_reduction: i32,
    pub use_fps_limit: bool,
    pub enable_dynamic_lod: bool,
    pub use_trees: bool,
}

impl Default for StaticGameLodInfo {
    /// `StaticGameLODInfo::StaticGameLODInfo()`.
    fn default() -> Self {
        StaticGameLodInfo {
            min_fps: 0,
            min_processor_fps: 0,
            sample_count_2d: 6,
            sample_count_3d: 24,
            stream_count: 2,
            max_particle_count: 2500,
            use_shadow_volumes: true,
            use_shadow_decals: true,
            use_cloud_map: true,
            use_light_map: true,
            show_soft_water_edge: true,
            max_tank_track_edges: 100,
            max_tank_track_opaque_edges: 25,
            max_tank_track_fade_delay: 300000,
            use_buildup_scaffolds: true,
            use_tree_sway: true,
            use_emissive_night_materials: true,
            use_heat_effects: true,
            texture_reduction: 0,
            use_fps_limit: true,
            enable_dynamic_lod: true,
            use_trees: true,
        }
    }
}

/// `struct DynamicGameLODInfo`.
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicGameLodInfo {
    pub min_fps: i32,
    /// `UnsignedInt`, parsed with `INI::parseInt`.
    pub dynamic_particle_skip_mask: u32,
    pub dynamic_debris_skip_mask: u32,
    pub slow_death_scale: f32,
    /// `ParticlePriorityType`.
    pub min_dynamic_particle_priority: i32,
    pub min_dynamic_particle_skip_priority: i32,
}

impl Default for DynamicGameLodInfo {
    /// `DynamicGameLODInfo::DynamicGameLODInfo()`.
    fn default() -> Self {
        DynamicGameLodInfo {
            min_fps: 0,
            dynamic_particle_skip_mask: 0,
            dynamic_debris_skip_mask: 0,
            slow_death_scale: 1.0,
            min_dynamic_particle_priority: priority::PARTICLE_PRIORITY_LOWEST,
            min_dynamic_particle_skip_priority: priority::PARTICLE_PRIORITY_LOWEST,
        }
    }
}

/// `struct LODPresetInfo`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodPresetInfo {
    pub cpu_type: i32,
    pub mhz: i32,
    /// `m_cpuPerfIndex` (never parsed nor read).
    pub cpu_perf_index: f32,
    pub video_type: i32,
    pub memory: i32,
}

impl Default for LodPresetInfo {
    fn default() -> Self {
        LodPresetInfo { cpu_type: cpu::XX, mhz: 1, cpu_perf_index: 1.0, video_type: chipset::DC_UNKNOWN, memory: 1 }
    }
}

/// `struct BenchProfile`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BenchProfile {
    pub cpu_type: i32,
    pub mhz: i32,
    pub int_bench_index: f32,
    pub float_bench_index: f32,
    pub mem_bench_index: f32,
}

impl Default for BenchProfile {
    fn default() -> Self {
        BenchProfile { cpu_type: cpu::XX, mhz: 1, int_bench_index: 1.0, float_bench_index: 1.0, mem_bench_index: 1.0 }
    }
}

/// What the LOD manager asks of the machine and the rest of the game (see the module docs).
pub trait GameLodHost {
    /// `testMinimumRequirements(&videoChipType, NULL, ...)`: the chip (`DC_UNKNOWN` when the device does not know it).
    fn video_chip_type(&mut self) -> i32;
    /// `testMinimumRequirements(NULL, &cpuType, &cpuFreq, &numRAM, NULL, NULL, NULL)`: the processor (`XX` unknown),
    /// its speed in MHz and the memory in bytes (`None`: unknown, the manager's value left unwritten).
    fn cpu_freq_ram(&mut self) -> (i32, Option<i32>, Option<i32>);
    /// `testMinimumRequirements(..., &intBenchIndex, &floatBenchIndex, &memBenchIndex)` (`RunBenchmark`): the three
    /// indices, integer, float, memory (`None`: not run, the manager's values left unwritten).
    fn benchmark(&mut self) -> Option<(f32, f32, f32)>;
    /// `fopen("Benchmark.txt", "w")` and its `fprintf` (with `m_forceBenchmark`): the text written.
    fn write_benchmark_file(&mut self, text: &[u8]);
    /// `TheGameClient->adjustLOD(adj)`.
    fn adjust_lod(&mut self, adj: i32);
    /// `TheGameClient->releaseShadows()` then `allocateShadows()`.
    fn reallocate_shadows(&mut self);
    /// `TheTerrainVisual->setShoreLineDetail()`.
    fn set_shore_line_detail(&mut self);
    /// `TheTerrainVisual->setTerrainTracksDetail()`.
    fn set_terrain_tracks_detail(&mut self);
    /// `findStaticLODLevel`'s `OptionPreferences`: `IdealStaticGameLOD` set to `ideal`, and `StaticGameLOD` to
    /// `static_level` when given, then `write()`.
    fn save_lod_preferences(&mut self, ideal: &[u8], static_level: Option<&[u8]>);
}

/// `OptionPreferences`' answers `GameLODManager::init` reads (`Options.ini`, OptionsMenu.cpp's getters).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LodCustomPrefs {
    /// `getTextureReduction()` (-1 without the entry).
    pub texture_reduction: i32,
    pub shadows_3d: bool,
    pub shadows_2d: bool,
    pub building_occlusion: bool,
    pub particle_cap: i32,
    pub dynamic_lod: bool,
    pub fps_limit: bool,
    pub lightmap: bool,
    pub cloud_shadows: bool,
    pub smooth_water: bool,
    pub heat_effects: bool,
    pub extra_animations_disabled: bool,
    pub trees: bool,
}

/// The `OptionPreferences` `GameLODManager::init` makes.
pub trait LodOptionPrefs {
    /// The `StaticGameLOD` entry, if there is one.
    fn static_game_lod(&self) -> Option<Vec<u8>>;
    /// The `IdealStaticGameLOD` entry, if there is one.
    fn ideal_static_game_lod(&self) -> Option<Vec<u8>>;
    /// The custom level's getters, each with `g` (`TheGlobalData`) as its default.
    fn custom(&mut self, g: &GlobalData) -> LodCustomPrefs;
}

/// The client's [`GameLodHost`] for a machine it does not measure: every hardware question answered *unknown*
/// (`XX`, `DC_UNKNOWN`, no speed, memory or benchmark) and recorded in [`UnknownHardware::calls`], with the other
/// calls; the preferences the manager saves are kept for the owner ([`UnknownHardware::saved`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnknownHardware {
    /// The calls, in order (`testMinimumRequirements(video)`, `adjustLOD(0)`, ...).
    pub calls: Vec<String>,
    /// `findStaticLODLevel`'s saves: (ideal, static level).
    pub saved: Vec<(Vec<u8>, Option<Vec<u8>>)>,
}

impl GameLodHost for UnknownHardware {
    fn video_chip_type(&mut self) -> i32 {
        self.calls.push("testMinimumRequirements(video): unknown".into());
        chipset::DC_UNKNOWN
    }
    fn cpu_freq_ram(&mut self) -> (i32, Option<i32>, Option<i32>) {
        self.calls.push("testMinimumRequirements(cpu, freq, RAM): unknown".into());
        (cpu::XX, None, None)
    }
    fn benchmark(&mut self) -> Option<(f32, f32, f32)> {
        self.calls.push("testMinimumRequirements(benchmark): not run".into());
        None
    }
    fn write_benchmark_file(&mut self, text: &[u8]) {
        self.calls.push(format!("Benchmark.txt: {}", String::from_utf8_lossy(text)));
    }
    fn adjust_lod(&mut self, adj: i32) {
        self.calls.push(format!("GameClient::adjustLOD({adj})"));
    }
    fn reallocate_shadows(&mut self) {
        self.calls.push("GameClient::releaseShadows, allocateShadows".into());
    }
    fn set_shore_line_detail(&mut self) {
        self.calls.push("TerrainVisual::setShoreLineDetail".into());
    }
    fn set_terrain_tracks_detail(&mut self) {
        self.calls.push("TerrainVisual::setTerrainTracksDetail".into());
    }
    fn save_lod_preferences(&mut self, ideal: &[u8], static_level: Option<&[u8]>) {
        self.saved.push((ideal.to_vec(), static_level.map(<[u8]>::to_vec)));
    }
}

/// `class GameLODManager`.
#[derive(Clone, Debug, PartialEq)]
pub struct GameLodManager {
    pub static_game_lod_info: [StaticGameLodInfo; 4],
    pub dynamic_game_lod_info: [DynamicGameLodInfo; 4],
    /// `m_lodPresets[STATIC_GAME_LOD_CUSTOM][MAX_LOD_PRESETS_PER_LEVEL]`.
    pub lod_presets: [[LodPresetInfo; MAX_LOD_PRESETS_PER_LEVEL]; 3],
    pub bench_profiles: [BenchProfile; MAX_BENCH_PROFILES],
    pub current_static_lod: i32,
    pub current_dynamic_lod: i32,
    pub num_particle_generations: i32,
    pub dynamic_particle_skip_mask: i32,
    pub num_debris_generations: i32,
    pub dynamic_debris_skip_mask: i32,
    pub slow_death_scale: f32,
    /// `m_minDynamicParticlePriority` (`None`: never set; the constructor leaves it uninitialised).
    pub min_dynamic_particle_priority: Option<i32>,
    /// `m_minDynamicParticleSkipPriority` (as above).
    pub min_dynamic_particle_skip_priority: Option<i32>,
    pub video_passed: bool,
    pub cpu_passed: bool,
    pub mem_passed: bool,
    pub num_level_presets: [i32; 3],
    pub num_bench_profiles: i32,
    pub ideal_detail_level: i32,
    pub video_chip_type: i32,
    pub cpu_type: i32,
    pub num_ram: i32,
    pub cpu_freq: i32,
    pub int_bench_index: f32,
    pub float_bench_index: f32,
    pub mem_bench_index: f32,
    pub composite_bench_index: f32,
    pub current_texture_reduction: i32,
    pub really_low_mhz: i32,
    /// The first undefined step upstream took (see the module docs); nothing is done for it.
    pub undefined: Option<&'static str>,
}

impl Default for GameLodManager {
    fn default() -> Self {
        Self::new()
    }
}

/// `INI::parseStaticGameLODLevel`'s and the manager's name match: `stricmp` / `AsciiString::compareNoCase`.
fn index_of(names: &[&str], name: &[u8]) -> Option<i32> {
    names.iter().position(|n| n.as_bytes().eq_ignore_ascii_case(name)).map(|i| i as i32)
}

impl GameLodManager {
    /// `GameLODManager::GameLODManager()`.
    pub fn new() -> Self {
        GameLodManager {
            static_game_lod_info: Default::default(),
            dynamic_game_lod_info: Default::default(),
            lod_presets: [[LodPresetInfo::default(); MAX_LOD_PRESETS_PER_LEVEL]; 3],
            bench_profiles: [BenchProfile::default(); MAX_BENCH_PROFILES],
            current_static_lod: STATIC_GAME_LOD_UNKNOWN,
            current_dynamic_lod: DYNAMIC_GAME_LOD_HIGH,
            num_particle_generations: 0,
            dynamic_particle_skip_mask: 0,
            num_debris_generations: 0,
            dynamic_debris_skip_mask: 0,
            slow_death_scale: 1.0,
            min_dynamic_particle_priority: None,
            min_dynamic_particle_skip_priority: None,
            video_passed: false,
            cpu_passed: false,
            mem_passed: false,
            num_level_presets: [0; 3],
            num_bench_profiles: 0,
            ideal_detail_level: STATIC_GAME_LOD_UNKNOWN,
            video_chip_type: chipset::DC_MAX,
            cpu_type: cpu::XX,
            num_ram: 0,
            cpu_freq: 0,
            int_bench_index: 0.0,
            float_bench_index: 0.0,
            mem_bench_index: 0.0,
            composite_bench_index: 0.0,
            current_texture_reduction: 0,
            really_low_mhz: 400,
            undefined: None,
        }
    }

    fn set_undefined(&mut self, what: &'static str) {
        if self.undefined.is_none() {
            self.undefined = Some(what);
        }
    }

    /// `getStaticGameLODLevelName(level)` (`None`: outside the table, undefined upstream).
    pub fn get_static_game_lod_level_name(&mut self, level: i32) -> Option<&'static str> {
        let n = usize::try_from(level).ok().and_then(|i| STATIC_GAME_LOD_NAMES.get(i).copied());
        if n.is_none() {
            self.set_undefined("getStaticGameLODLevelName: StaticGameLODNames[level] outside the table");
        }
        n
    }

    /// `getDynamicGameLODLevelName(level)` (`None`: outside the table, undefined upstream).
    pub fn get_dynamic_game_lod_level_name(&mut self, level: i32) -> Option<&'static str> {
        let n = usize::try_from(level).ok().and_then(|i| DYNAMIC_GAME_LOD_NAMES.get(i).copied());
        if n.is_none() {
            self.set_undefined("getDynamicGameLODLevelName: DynamicGameLODNames[level] outside the table");
        }
        n
    }

    /// `getStaticGameLODIndex(name)`: the level named (any case), else `STATIC_GAME_LOD_UNKNOWN` (`DEBUG_CRASH` only in
    /// debug builds).
    pub fn get_static_game_lod_index(&self, name: &[u8]) -> i32 {
        index_of(&STATIC_GAME_LOD_NAMES, name).unwrap_or(STATIC_GAME_LOD_UNKNOWN)
    }

    /// `getDynamicGameLODIndex(name)`: the level named, else `STATIC_GAME_LOD_UNKNOWN` (-1, as upstream returns).
    pub fn get_dynamic_game_lod_index(&self, name: &[u8]) -> i32 {
        index_of(&DYNAMIC_GAME_LOD_NAMES, name).unwrap_or(STATIC_GAME_LOD_UNKNOWN)
    }

    /// `getStaticLODLevel()`.
    pub fn get_static_lod_level(&self) -> i32 {
        self.current_static_lod
    }

    /// `getDynamicLODLevel()`.
    pub fn get_dynamic_lod_level(&self) -> i32 {
        self.current_dynamic_lod
    }

    /// `setCurrentTextureReduction(val)`.
    pub fn set_current_texture_reduction(&mut self, val: i32) {
        self.current_texture_reduction = val;
    }

    /// `getCurrentTextureReduction()`.
    pub fn get_current_texture_reduction(&self) -> i32 {
        self.current_texture_reduction
    }

    /// `isParticleSkipped()`: `(++m_numParticleGenerations & m_dynamicParticleSkipMask) != m_dynamicParticleSkipMask`.
    pub fn is_particle_skipped(&mut self) -> bool {
        if plant(9549) {
            let r = (self.num_particle_generations & self.dynamic_particle_skip_mask) != self.dynamic_particle_skip_mask;
            self.num_particle_generations = self.num_particle_generations.wrapping_add(1);
            return r;
        }
        self.num_particle_generations = self.num_particle_generations.wrapping_add(1);
        (self.num_particle_generations & self.dynamic_particle_skip_mask) != self.dynamic_particle_skip_mask
    }

    /// `isDebrisSkipped()`.
    pub fn is_debris_skipped(&mut self) -> bool {
        self.num_debris_generations = self.num_debris_generations.wrapping_add(1);
        (self.num_debris_generations & self.dynamic_debris_skip_mask) != self.dynamic_debris_skip_mask
    }

    /// `getSlowDeathScale()`.
    pub fn get_slow_death_scale(&self) -> f32 {
        self.slow_death_scale
    }

    /// `getMinDynamicParticlePriority()` (`None`: never set, undefined upstream).
    pub fn get_min_dynamic_particle_priority(&mut self) -> Option<i32> {
        if self.min_dynamic_particle_priority.is_none() {
            self.set_undefined("getMinDynamicParticlePriority: m_minDynamicParticlePriority never set (uninitialised)");
        }
        self.min_dynamic_particle_priority
    }

    /// `getMinDynamicParticleSkipPriority()` (`None`: never set, undefined upstream).
    pub fn get_min_dynamic_particle_skip_priority(&mut self) -> Option<i32> {
        if self.min_dynamic_particle_skip_priority.is_none() {
            self.set_undefined("getMinDynamicParticleSkipPriority: m_minDynamicParticleSkipPriority never set (uninitialised)");
        }
        self.min_dynamic_particle_skip_priority
    }

    /// `newBenchProfile()`: the next of the 16 (`None` past them: `DEBUG_CRASH` only in debug builds).
    pub fn new_bench_profile(&mut self) -> Option<&mut BenchProfile> {
        if (self.num_bench_profiles as usize) < MAX_BENCH_PROFILES {
            self.num_bench_profiles += 1;
            return Some(&mut self.bench_profiles[self.num_bench_profiles as usize - 1]);
        }
        None
    }

    /// `newLODPreset(index)`: the next of the level's 32 (`None` past them). `index` must be low, medium or high
    /// (the caller checks: upstream indexes `m_numLevelPresets[index]` unchecked).
    pub fn new_lod_preset(&mut self, index: i32) -> Option<&mut LodPresetInfo> {
        let i = index as usize;
        if (self.num_level_presets[i] as usize) < MAX_LOD_PRESETS_PER_LEVEL {
            self.num_level_presets[i] += 1;
            return Some(&mut self.lod_presets[i][self.num_level_presets[i] as usize - 1]);
        }
        None
    }

    /// `didMemPass()`.
    pub fn did_mem_pass(&self) -> bool {
        self.mem_passed
    }

    /// `setReallyLowMHz(mhz)`.
    pub fn set_really_low_mhz(&mut self, mhz: i32) {
        self.really_low_mhz = mhz;
    }

    /// `isReallyLowMHz()`.
    pub fn is_really_low_mhz(&self) -> bool {
        self.cpu_freq < self.really_low_mhz
    }

    /// `GameLODManager::init()`: the two INI files loaded (`files(path)`: the file's bytes, `None` when it is
    /// missing: `INI_CANT_OPEN_FILE`, which ends `init` as upstream's throw would), the custom level refreshed from
    /// `g`, the player's level read from `prefs`, the hardware asked of `host`, the benchmark matched against the
    /// profiles when the processor is unknown (or `m_forceBenchmark`), the custom options applied to `g` for a custom
    /// level, and the player's level set.
    pub fn init<H: GameLodHost, P: LodOptionPrefs>(&mut self, host: &mut H, g: &mut GlobalData, prefs: &mut P, files: impl Fn(&[u8]) -> Option<Vec<u8>>) -> IniResult<()> {
        for path in [&b"Data\\INI\\GameLOD.ini"[..], b"Data\\INI\\GameLODPresets.ini"] {
            let data = files(path).ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::CantOpenFile))?;
            Ini::load(path, &data, crate::common::ini::IniLoadType::Overwrite, None, game_lod_blocks(), self)?;
        }
        self.refresh_custom_static_lod_level(g);
        // OptionPreferences::getStaticGameDetail, getIdealStaticGameDetail (OptionsMenu.cpp)
        let user_set_detail = prefs.static_game_lod().map_or(self.get_static_lod_level(), |v| self.get_static_game_lod_index(&v));
        self.ideal_detail_level = prefs.ideal_static_game_lod().map_or(STATIC_GAME_LOD_UNKNOWN, |v| self.get_static_game_lod_index(&v));
        let (cpu_type, freq, ram) = host.cpu_freq_ram();
        self.cpu_type = cpu_type;
        if let Some(f) = freq {
            self.cpu_freq = f;
        }
        if let Some(r) = ram {
            self.num_ram = r;
        }
        if self.num_ram as f32 / (256 * 1024 * 1024) as f32 >= PROFILE_ERROR_LIMIT {
            self.mem_passed = true;
        }
        // (upstream's two nested tests: the data for the ideal level wanted, and the processor unknown)
        if (self.ideal_detail_level == STATIC_GAME_LOD_UNKNOWN || g.force_benchmark) && (self.cpu_type == cpu::XX || g.force_benchmark) {
            if let Some((i, f, m)) = host.benchmark() {
                // (RunBenchmark(0, NULL, floatBenchIndex, intBenchIndex, memBenchIndex))
                self.int_bench_index = i;
                self.float_bench_index = f;
                self.mem_bench_index = m;
            }
            if g.force_benchmark {
                let Some(name) = usize::try_from(self.cpu_type).ok().and_then(|i| CPU_NAMES.get(i)) else {
                    self.set_undefined("init: CPUNames[m_cpuType] outside the table");
                    return Ok(());
                };
                let text = format!("BenchProfile = {} {} {:.6} {:.6} {:.6}", name, self.cpu_freq, f64::from(self.int_bench_index), f64::from(self.float_bench_index), f64::from(self.mem_bench_index));
                host.write_benchmark_file(text.as_bytes());
            }
            self.composite_bench_index = self.int_bench_index + self.float_bench_index;
            let mut current_level = STATIC_GAME_LOD_LOW;
            self.cpu_type = cpu::P3;
            self.cpu_freq = 1000;
            for k in 0..self.num_bench_profiles as usize {
                let prof = self.bench_profiles[k];
                if self.int_bench_index / prof.int_bench_index >= PROFILE_ERROR_LIMIT
                    && self.float_bench_index / prof.float_bench_index >= PROFILE_ERROR_LIMIT
                    && (plant(9551) || self.mem_bench_index / prof.mem_bench_index >= PROFILE_ERROR_LIMIT)
                {
                    let mut i = STATIC_GAME_LOD_HIGH;
                    while i >= STATIC_GAME_LOD_LOW {
                        for j in 0..self.num_level_presets[i as usize] as usize {
                            let preset = self.lod_presets[i as usize][j];
                            if prof.cpu_type == preset.cpu_type && prof.mhz as f32 / preset.mhz as f32 >= PROFILE_ERROR_LIMIT {
                                current_level = i;
                                self.cpu_type = prof.cpu_type;
                                self.cpu_freq = prof.mhz;
                                break;
                            }
                        }
                        if current_level >= i {
                            break;
                        }
                        i -= 1;
                    }
                }
            }
        }
        if user_set_detail == STATIC_GAME_LOD_CUSTOM {
            let c = prefs.custom(g);
            g.texture_reduction_factor = c.texture_reduction;
            g.use_shadow_volumes = c.shadows_3d;
            g.use_shadow_decals = c.shadows_2d;
            g.enable_behind_building_markers = c.building_occlusion;
            g.max_particle_count = c.particle_cap;
            g.enable_dynamic_lod = c.dynamic_lod;
            g.use_fps_limit = c.fps_limit;
            g.use_light_map = c.lightmap;
            g.use_cloud_map = c.cloud_shadows;
            g.show_soft_water_edge = c.smooth_water;
            g.use_heat_effects = c.heat_effects;
            g.use_draw_module_lod = c.extra_animations_disabled;
            g.use_tree_sway = !g.use_draw_module_lod != plant(9552);
            g.use_trees = c.trees;
        }
        self.set_static_lod_level(host, g, user_set_detail);
        Ok(())
    }

    /// `refreshCustomStaticLODLevel()`: the custom level is the global data's settings.
    pub fn refresh_custom_static_lod_level(&mut self, g: &GlobalData) {
        let l = &mut self.static_game_lod_info[STATIC_GAME_LOD_CUSTOM as usize];
        l.max_particle_count = g.max_particle_count;
        l.use_shadow_volumes = g.use_shadow_volumes;
        l.use_shadow_decals = g.use_shadow_decals;
        l.use_cloud_map = g.use_cloud_map;
        l.use_light_map = g.use_light_map;
        l.show_soft_water_edge = g.show_soft_water_edge;
        l.max_tank_track_edges = g.max_tank_track_edges;
        l.max_tank_track_opaque_edges = g.max_tank_track_opaque_edges;
        l.max_tank_track_fade_delay = g.max_tank_track_fade_delay;
        l.use_buildup_scaffolds = !g.use_draw_module_lod != plant(9556);
        l.use_heat_effects = g.use_heat_effects;
        l.use_tree_sway = l.use_buildup_scaffolds;
        l.texture_reduction = g.texture_reduction_factor;
        l.use_fps_limit = g.use_fps_limit;
        l.enable_dynamic_lod = g.enable_dynamic_lod;
        l.use_trees = g.use_trees;
    }

    /// `findStaticLODLevel()`: the recommended level for this machine, matched once against the presets (the video
    /// chip asked of `host`, an unknown one presumed a TNT2) and saved in `Options.ini` (`IdealStaticGameLOD`, and
    /// `StaticGameLOD` while no level is set).
    pub fn find_static_lod_level<H: GameLodHost>(&mut self, host: &mut H) -> i32 {
        if self.ideal_detail_level == STATIC_GAME_LOD_UNKNOWN {
            self.ideal_detail_level = STATIC_GAME_LOD_LOW;
            self.video_chip_type = host.video_chip_type();
            if self.video_chip_type == chipset::DC_UNKNOWN {
                self.video_chip_type = if plant(9543) { chipset::DC_TNT } else { chipset::DC_TNT2 };
            }
            let limit = if plant(9541) { 0.95 } else { PROFILE_ERROR_LIMIT };
            let num_mb_ram = self.num_ram / (1024 * 1024);
            let mut i = STATIC_GAME_LOD_HIGH;
            while i >= STATIC_GAME_LOD_LOW {
                for j in 0..self.num_level_presets[i as usize] as usize {
                    let preset = self.lod_presets[i as usize][j];
                    if self.cpu_type == preset.cpu_type
                        && self.cpu_freq as f32 / preset.mhz as f32 >= limit
                        && (if plant(9542) { self.video_chip_type > preset.video_type } else { self.video_chip_type >= preset.video_type })
                        && num_mb_ram as f32 / preset.memory as f32 >= limit
                    {
                        self.ideal_detail_level = i;
                        break;
                    }
                }
                if self.ideal_detail_level >= i {
                    break;
                }
                i -= 1;
            }
            let ideal = STATIC_GAME_LOD_NAMES[self.ideal_detail_level as usize].as_bytes();
            let static_level = (self.get_static_lod_level() == STATIC_GAME_LOD_UNKNOWN).then_some(ideal);
            host.save_lod_preferences(ideal, static_level);
        }
        self.ideal_detail_level
    }

    /// `setStaticLODLevel(level)`: whether it was applied (not with `m_enableStaticLOD` off, which makes the level
    /// custom; not for unknown; not again for the level already set, but for custom, which is always applied).
    pub fn set_static_lod_level<H: GameLodHost>(&mut self, host: &mut H, g: &mut GlobalData, level: i32) -> bool {
        if !g.enable_static_lod {
            self.current_static_lod = STATIC_GAME_LOD_CUSTOM;
            return false;
        }
        if level == STATIC_GAME_LOD_UNKNOWN || ((level != STATIC_GAME_LOD_CUSTOM || plant(9544)) && self.current_static_lod == level) {
            return false;
        }
        if !(STATIC_GAME_LOD_LOW..STATIC_GAME_LOD_COUNT).contains(&level) {
            self.set_undefined("setStaticLODLevel: m_staticGameLODInfo[level] outside the table");
            return false;
        }
        self.apply_static_lod_level(host, g, level);
        self.current_static_lod = level;
        true
    }

    /// `applyStaticLODLevel(level)`: the level's settings written into `g`, the client and terrain told what changed.
    fn apply_static_lod_level<H: GameLodHost>(&mut self, host: &mut H, g: &mut GlobalData, level: i32) {
        let prev = if self.current_static_lod != STATIC_GAME_LOD_UNKNOWN {
            self.static_game_lod_info[self.current_static_lod as usize].clone()
        } else {
            StaticGameLodInfo::default()
        };
        if level == STATIC_GAME_LOD_CUSTOM {
            self.refresh_custom_static_lod_level(g);
        }
        let mut requested_texture_reduction = 0;
        let mut requested_trees = self.mem_passed;
        if level == STATIC_GAME_LOD_CUSTOM {
            let l = &self.static_game_lod_info[level as usize];
            requested_texture_reduction = l.texture_reduction;
            requested_trees = l.use_trees;
        } else if level >= STATIC_GAME_LOD_LOW {
            requested_texture_reduction = self.get_recommended_texture_reduction(host);
            if plant(9545) {
                requested_trees = self.static_game_lod_info[level as usize].use_trees;
            }
        }
        let l = self.static_game_lod_info[level as usize].clone();
        g.max_particle_count = l.max_particle_count;
        g.use_shadow_volumes = l.use_shadow_volumes;
        g.use_shadow_decals = l.use_shadow_decals;
        if requested_texture_reduction != self.current_texture_reduction {
            g.texture_reduction_factor = requested_texture_reduction;
            host.adjust_lod(0);
        }
        if self.current_static_lod == STATIC_GAME_LOD_UNKNOWN || l.use_shadow_volumes != prev.use_shadow_volumes || (l.use_shadow_decals != prev.use_shadow_decals && !plant(9547)) {
            host.reallocate_shadows();
        }
        g.use_cloud_map = l.use_cloud_map;
        g.use_light_map = l.use_light_map;
        g.show_soft_water_edge = l.show_soft_water_edge;
        if self.current_static_lod == STATIC_GAME_LOD_UNKNOWN || l.show_soft_water_edge != prev.show_soft_water_edge {
            host.set_shore_line_detail();
        }
        g.max_tank_track_edges = l.max_tank_track_edges;
        g.max_tank_track_opaque_edges = l.max_tank_track_opaque_edges;
        g.max_tank_track_fade_delay = l.max_tank_track_fade_delay;
        g.use_tree_sway = l.use_tree_sway;
        g.use_draw_module_lod = !l.use_buildup_scaffolds;
        g.use_heat_effects = l.use_heat_effects;
        g.enable_dynamic_lod = l.enable_dynamic_lod;
        g.use_fps_limit = l.use_fps_limit;
        g.use_trees = requested_trees;
        if !self.mem_passed || (self.is_really_low_mhz() && !plant(9546)) {
            g.shell_map_on = false;
        }
        host.set_terrain_tracks_detail();
    }

    /// `findDynamicLODLevel(averageFPS)`: the highest level whose minimum is below the rate (as an `Int`), else low
    /// (`None`: the rate is no `Int`, undefined upstream).
    pub fn find_dynamic_lod_level(&mut self, average_fps: f32) -> Option<i32> {
        // `(Int)(averageFPS)`: truncation, undefined outside Int
        if !(average_fps > -2147483649.0f32 && average_fps < 2147483648.0f32) {
            self.set_undefined("findDynamicLODLevel: (Int)averageFPS outside Int");
            return None;
        }
        let ifps = average_fps as i32;
        let mut i = DYNAMIC_GAME_LOD_VERY_HIGH;
        while i >= DYNAMIC_GAME_LOD_LOW {
            if self.dynamic_game_lod_info[i as usize].min_fps < ifps || (plant(9548) && self.dynamic_game_lod_info[i as usize].min_fps == ifps) {
                return Some(i);
            }
            i -= 1;
        }
        Some(DYNAMIC_GAME_LOD_LOW)
    }

    /// `setDynamicLODLevel(level)`: whether it changed (and was applied).
    pub fn set_dynamic_lod_level(&mut self, level: i32) -> bool {
        if level == DYNAMIC_GAME_LOD_UNKNOWN || self.current_dynamic_lod == level {
            return false;
        }
        if !(DYNAMIC_GAME_LOD_LOW..DYNAMIC_GAME_LOD_COUNT).contains(&level) {
            self.set_undefined("setDynamicLODLevel: m_dynamicGameLODInfo[level] outside the table");
            return false;
        }
        self.current_dynamic_lod = level;
        self.apply_dynamic_lod_level(level);
        true
    }

    /// `applyDynamicLODLevel(level)`.
    fn apply_dynamic_lod_level(&mut self, level: i32) {
        let d = self.dynamic_game_lod_info[level as usize].clone();
        if !plant(9550) {
            self.num_particle_generations = 0;
        }
        self.dynamic_particle_skip_mask = d.dynamic_particle_skip_mask as i32;
        self.num_debris_generations = 0;
        self.dynamic_debris_skip_mask = d.dynamic_debris_skip_mask as i32;
        self.slow_death_scale = d.slow_death_scale;
        self.min_dynamic_particle_priority = Some(d.min_dynamic_particle_priority);
        self.min_dynamic_particle_skip_priority = Some(d.min_dynamic_particle_skip_priority);
    }

    /// `getRecommendedTextureReduction()`: low's with less than 256 MB, else the ideal level's (found first if it
    /// never was).
    pub fn get_recommended_texture_reduction<H: GameLodHost>(&mut self, host: &mut H) -> i32 {
        if self.ideal_detail_level == STATIC_GAME_LOD_UNKNOWN {
            self.find_static_lod_level(host);
        }
        if !self.mem_passed && !plant(9554) {
            return self.static_game_lod_info[STATIC_GAME_LOD_LOW as usize].texture_reduction;
        }
        self.static_game_lod_info[self.ideal_detail_level as usize].texture_reduction
    }

    /// `getLevelTextureReduction(level)` (`None`: outside the table, undefined upstream).
    pub fn get_level_texture_reduction(&mut self, level: i32) -> Option<i32> {
        match usize::try_from(level).ok().and_then(|i| self.static_game_lod_info.get(i)) {
            Some(l) => Some(l.texture_reduction),
            None => {
                self.set_undefined("getLevelTextureReduction: m_staticGameLODInfo[level] outside the table");
                None
            }
        }
    }

    /// `W3DDisplay::init`'s static LOD: a level never set is set to the one found for this machine; else the texture
    /// reduction the display would apply (`WW3D::Set_Texture_Reduction`, not translated: the client's textures are not
    /// reduced, so the manager is not told it was applied). Whether the level was set.
    pub fn display_init<H: GameLodHost>(&mut self, host: &mut H, g: &mut GlobalData) -> bool {
        if self.get_static_lod_level() == STATIC_GAME_LOD_UNKNOWN {
            let level = self.find_static_lod_level(host);
            return self.set_static_lod_level(host, g, level);
        }
        false
    }

    /// `W3DDisplay::draw`'s dynamic LOD: the level for `average_fps` while `m_enableDynamicLOD` and
    /// `TheGameLogic->getShowDynamicLOD()`, else the highest.
    pub fn display_draw(&mut self, enable_dynamic_lod: bool, show_dynamic_lod: bool, average_fps: f32) {
        if enable_dynamic_lod && show_dynamic_lod {
            if let Some(lod) = self.find_dynamic_lod_level(average_fps) {
                self.set_dynamic_lod_level(lod);
            }
        } else {
            self.set_dynamic_lod_level(DYNAMIC_GAME_LOD_VERY_HIGH);
        }
    }
}

/// `W3DDisplay::updateAverageFPS`'s statics and `m_averageFPS`: the frame rate averaged over the last 30 frames whose
/// interval was at most half a second.
#[derive(Clone, Debug, PartialEq)]
pub struct AverageFps {
    pub last_update_time: i64,
    pub history_offset: i32,
    pub num_samples: i32,
    pub fps_history: [f64; 30],
    /// `m_averageFPS`.
    pub average_fps: f32,
}

impl Default for AverageFps {
    fn default() -> Self {
        AverageFps { last_update_time: 0, history_offset: 0, num_samples: 0, fps_history: [0.0; 30], average_fps: 0.0 }
    }
}

impl AverageFps {
    /// `W3DDisplay::updateAverageFPS()` at `time` ticks of a counter of `freq` ticks a second
    /// (`getPerformanceCounter`, `getPerformanceCounterFrequency`).
    pub fn update(&mut self, time: i64, freq: i64) {
        const MAXIMUM_FRAME_TIME_CUTOFF: f32 = 0.5;
        const FPS_HISTORY_SIZE: i32 = 30;
        let time_diff = time.wrapping_sub(self.last_update_time);
        let elapsed_seconds = time_diff as f64 / freq as f64;
        if elapsed_seconds <= f64::from(MAXIMUM_FRAME_TIME_CUTOFF) && !(plant(9555) && elapsed_seconds == f64::from(MAXIMUM_FRAME_TIME_CUTOFF)) {
            if self.history_offset >= FPS_HISTORY_SIZE {
                self.history_offset = 0;
            }
            let current_fps = 1.0 / elapsed_seconds;
            self.fps_history[self.history_offset as usize] = current_fps;
            self.history_offset += 1;
            self.num_samples += 1;
            if self.num_samples > FPS_HISTORY_SIZE {
                self.num_samples = FPS_HISTORY_SIZE;
            }
        }
        if self.num_samples != 0 {
            let mut average: f32 = 0.0;
            let mut j = self.history_offset - 1;
            for _ in 0..self.num_samples {
                if j < 0 {
                    j = FPS_HISTORY_SIZE - 1;
                }
                // (`Real += double`: the sum in double, stored back as float)
                average = (f64::from(average) + self.fps_history[j as usize]) as f32;
                j -= 1;
            }
            self.average_fps = average / self.num_samples as f32;
        }
        self.last_update_time = time;
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The INI blocks
// ---------------------------------------------------------------------------------------------------------------------

/// The field tables' missing terminator: a token neither table holds (see the module docs).
fn unterminated<T>(_: &mut Ini<'_>, _: &mut T, _: &UserData) -> IniResult<()> {
    Err(IniError::undefined(Undefined::UnterminatedFieldTable))
}

/// `TheStaticGameLODFieldParseTable` (no terminator upstream: [`unterminated`] stands for what lies past it).
pub static STATIC_GAME_LOD_FIELDS: [FieldParse<StaticGameLodInfo>; 20] = [
    FieldParse::new("MinimumFPS", |i, l, _| i.parse_int().map(|v| l.min_fps = v), UserData::None),
    FieldParse::new("MinimumProcessorFps", |i, l, _| i.parse_int().map(|v| l.min_processor_fps = v), UserData::None),
    FieldParse::new("SampleCount2D", |i, l, _| i.parse_int().map(|v| l.sample_count_2d = v), UserData::None),
    FieldParse::new("SampleCount3D", |i, l, _| i.parse_int().map(|v| l.sample_count_3d = v), UserData::None),
    FieldParse::new("StreamCount", |i, l, _| i.parse_int().map(|v| l.stream_count = v), UserData::None),
    FieldParse::new("MaxParticleCount", |i, l, _| i.parse_int().map(|v| l.max_particle_count = v), UserData::None),
    FieldParse::new("UseShadowVolumes", |i, l, _| i.parse_bool().map(|v| l.use_shadow_volumes = v), UserData::None),
    FieldParse::new("UseShadowDecals", |i, l, _| i.parse_bool().map(|v| l.use_shadow_decals = v), UserData::None),
    FieldParse::new("UseCloudMap", |i, l, _| i.parse_bool().map(|v| l.use_cloud_map = v), UserData::None),
    FieldParse::new("UseLightMap", |i, l, _| i.parse_bool().map(|v| l.use_light_map = v), UserData::None),
    FieldParse::new("ShowSoftWaterEdge", |i, l, _| i.parse_bool().map(|v| l.show_soft_water_edge = v), UserData::None),
    FieldParse::new("MaxTankTrackEdges", |i, l, _| i.parse_int().map(|v| l.max_tank_track_edges = v), UserData::None),
    FieldParse::new("MaxTankTrackOpaqueEdges", |i, l, _| i.parse_int().map(|v| l.max_tank_track_opaque_edges = v), UserData::None),
    FieldParse::new("MaxTankTrackFadeDelay", |i, l, _| i.parse_int().map(|v| l.max_tank_track_fade_delay = v), UserData::None),
    FieldParse::new("UseBuildupScaffolds", |i, l, _| i.parse_bool().map(|v| l.use_buildup_scaffolds = v), UserData::None),
    FieldParse::new("UseTreeSway", |i, l, _| i.parse_bool().map(|v| l.use_tree_sway = v), UserData::None),
    FieldParse::new("UseEmissiveNightMaterials", |i, l, _| i.parse_bool().map(|v| l.use_emissive_night_materials = v), UserData::None),
    FieldParse::new("UseHeatEffects", |i, l, _| i.parse_bool().map(|v| l.use_heat_effects = v), UserData::None),
    FieldParse::new("TextureReductionFactor", |i, l, _| i.parse_int().map(|v| l.texture_reduction = v), UserData::None),
    FieldParse::catch_all(unterminated),
];

/// `TheDynamicGameLODFieldParseTable` (no terminator upstream, as the static one).
pub static DYNAMIC_GAME_LOD_FIELDS: [FieldParse<DynamicGameLodInfo>; 7] = [
    FieldParse::new("MinimumFPS", |i, l, _| i.parse_int().map(|v| l.min_fps = v), UserData::None),
    FieldParse::new("ParticleSkipMask", |i, l, _| i.parse_int().map(|v| l.dynamic_particle_skip_mask = v as u32), UserData::None),
    FieldParse::new("DebrisSkipMask", |i, l, _| i.parse_int().map(|v| l.dynamic_debris_skip_mask = v as u32), UserData::None),
    FieldParse::new("SlowDeathScale", |i, l, _| i.parse_real().map(|v| l.slow_death_scale = v), UserData::None),
    FieldParse::new("MinParticlePriority", |i, l, ud| i.parse_index_list(ud.names()).map(|v| l.min_dynamic_particle_priority = v), UserData::Names(PARTICLE_PRIORITY_NAMES)),
    FieldParse::new("MinParticleSkipPriority", |i, l, ud| i.parse_index_list(ud.names()).map(|v| l.min_dynamic_particle_skip_priority = v), UserData::Names(PARTICLE_PRIORITY_NAMES)),
    FieldParse::catch_all(unterminated),
];

/// `parseReallyLowMHz(ini)`: `ReallyLowMHz = <mhz>`.
pub fn parse_really_low_mhz(ini: &mut Ini<'_>, m: &mut GameLodManager) -> IniResult<()> {
    let mhz = ini.parse_int()?;
    m.set_really_low_mhz(mhz);
    Ok(())
}

/// `INI::parseBenchProfile(ini)`: `BenchProfile = <cpu> <mhz> <int> <float> <mem>` (nothing read past 16 profiles).
pub fn parse_bench_profile(ini: &mut Ini<'_>, m: &mut GameLodManager) -> IniResult<()> {
    if let Some(p) = m.new_bench_profile() {
        p.cpu_type = ini.parse_index_list(&CPU_NAMES)?;
        p.mhz = ini.parse_int()?;
        p.int_bench_index = ini.parse_real()?;
        p.float_bench_index = ini.parse_real()?;
        p.mem_bench_index = ini.parse_real()?;
    }
    Ok(())
}

/// `INI::parseLODPreset(ini)`: `LODPreset = <level> <cpu> <mhz> <video> <memory>` (an unknown level, or past 32
/// presets: nothing more read; the level `Custom` is undefined, see the module docs).
pub fn parse_lod_preset(ini: &mut Ini<'_>, m: &mut GameLodManager) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    let index = m.get_static_game_lod_index(&name);
    if index != STATIC_GAME_LOD_UNKNOWN {
        if index == STATIC_GAME_LOD_CUSTOM {
            return Err(IniError::undefined(Undefined::ArrayIndexOutOfRange));
        }
        if let Some(p) = m.new_lod_preset(index) {
            p.cpu_type = ini.parse_index_list(&CPU_NAMES)?;
            if plant(9553) {
                p.video_type = ini.parse_index_list(&VIDEO_NAMES)?;
                p.mhz = ini.parse_int()?;
            } else {
                p.mhz = ini.parse_int()?;
                p.video_type = ini.parse_index_list(&VIDEO_NAMES)?;
            }
            p.memory = ini.parse_int()?;
        }
    }
    Ok(())
}

/// `INI::parseStaticGameLODDefinition(ini)`: `StaticGameLOD <level>` and its fields (an unknown level: nothing more
/// read).
pub fn parse_static_game_lod_definition(ini: &mut Ini<'_>, m: &mut GameLodManager) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    let index = m.get_static_game_lod_index(&name);
    if index != STATIC_GAME_LOD_UNKNOWN {
        ini.init_from_ini(&mut m.static_game_lod_info[index as usize], &STATIC_GAME_LOD_FIELDS)?;
    }
    Ok(())
}

/// `INI::parseDynamicGameLODDefinition(ini)`: `DynamicGameLOD <level>` and its fields.
pub fn parse_dynamic_game_lod_definition(ini: &mut Ini<'_>, m: &mut GameLodManager) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    let index = m.get_dynamic_game_lod_index(&name);
    if index != DYNAMIC_GAME_LOD_UNKNOWN {
        ini.init_from_ini(&mut m.dynamic_game_lod_info[index as usize], &DYNAMIC_GAME_LOD_FIELDS)?;
    }
    Ok(())
}

/// `INI::parseDynamicGameLODLevel(ini, instance, store, userData)`: a field's dynamic level by name
/// (`INI_INVALID_DATA` for another). The static one is `crate::client::w3d_model_draw`'s.
pub fn parse_dynamic_game_lod_level(ini: &mut Ini<'_>) -> IniResult<i32> {
    let tok = ini.get_next_token(None)?;
    index_of(&DYNAMIC_GAME_LOD_NAMES, &tok).ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))
}

/// The five rows of `theTypeTable` GameLOD.cpp handles.
pub static GAME_LOD_BLOCKS: [BlockParse<GameLodManager>; 5] = [
    BlockParse { token: "StaticGameLOD", parse: parse_static_game_lod_definition },
    BlockParse { token: "DynamicGameLOD", parse: parse_dynamic_game_lod_definition },
    BlockParse { token: "LODPreset", parse: parse_lod_preset },
    BlockParse { token: "BenchProfile", parse: parse_bench_profile },
    BlockParse { token: "ReallyLowMHz", parse: parse_really_low_mhz },
];

/// [`GAME_LOD_BLOCKS`] as `INI::load`'s table: upstream's table is fixed, and in the files the LOD manager loads
/// any other block is an error of its own handler's; the client's loader knows only these five (another block:
/// `INI_UNKNOWN_TOKEN`).
pub fn game_lod_blocks() -> &'static [BlockParse<GameLodManager>] {
    &GAME_LOD_BLOCKS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{IniErrorKind, IniLoadType};

    /// A synthetic `GameLOD.ini` (the shape of the game's, no EA data) and `GameLODPresets.ini`.
    const LOD_INI: &[u8] = b"; synthetic\r\nStaticGameLOD = Low\r\n  MaxParticleCount = 500\r\n  UseShadowVolumes = No\r\n  TextureReductionFactor = 1\r\nEnd\r\nStaticGameLOD = High\r\n  MaxParticleCount = 3000\r\nEnd\r\nDynamicGameLOD = Low\r\n  MinimumFPS = 0\r\n  ParticleSkipMask = 3\r\n  SlowDeathScale = 0.5\r\n  MinParticlePriority = DEBRIS_TRAIL\r\nEnd\r\nDynamicGameLOD = VeryHigh\r\n  MinimumFPS = 25\r\nEnd\r\n";
    const PRESETS_INI: &[u8] = b"LODPreset = High P4 2000 GF3 512\r\nLODPreset = Medium P3 1000 TNT2 256\r\nBenchProfile = P4 2000 2.0 2.0 2.0\r\nReallyLowMHz = 600\r\n";

    struct Prefs(Option<Vec<u8>>);
    impl LodOptionPrefs for Prefs {
        fn static_game_lod(&self) -> Option<Vec<u8>> {
            self.0.clone()
        }
        fn ideal_static_game_lod(&self) -> Option<Vec<u8>> {
            None
        }
        fn custom(&mut self, _: &GlobalData) -> LodCustomPrefs {
            LodCustomPrefs::default()
        }
    }

    fn files(p: &[u8]) -> Option<Vec<u8>> {
        match p {
            b"Data\\INI\\GameLOD.ini" => Some(LOD_INI.to_vec()),
            b"Data\\INI\\GameLODPresets.ini" => Some(PRESETS_INI.to_vec()),
            _ => None,
        }
    }

    /// The client's unknown machine: the presets read, the benchmark not run (the profiles cannot match: P3 at 1000
    /// presumed), no memory (low textures, no trees), and the level from `Options.ini` applied.
    #[test]
    fn an_unknown_machine_takes_upstream_s_paths() {
        let mut m = GameLodManager::new();
        let mut h = UnknownHardware::default();
        let mut g = GlobalData::new();
        g.enable_static_lod = true;
        m.init(&mut h, &mut g, &mut Prefs(Some(b"high".to_vec())), files).expect("the files read");
        assert_eq!(m.num_level_presets, [0, 1, 1]);
        assert_eq!(m.num_bench_profiles, 1);
        assert_eq!(m.really_low_mhz, 600);
        assert_eq!((m.cpu_type, m.cpu_freq, m.num_ram, m.mem_passed), (cpu::P3, 1000, 0, false));
        assert_eq!(m.get_static_lod_level(), STATIC_GAME_LOD_HIGH);
        assert_eq!(g.max_particle_count, 3000);
        // the recommended texture reduction found the level (TNT2 presumed, P3 1000 but no memory: low) and is low's
        assert_eq!(m.ideal_detail_level, STATIC_GAME_LOD_LOW);
        assert_eq!(g.texture_reduction_factor, 1);
        assert!(!g.use_trees && !g.shell_map_on);
        assert_eq!(h.saved, vec![(b"Low".to_vec(), Some(b"Low".to_vec()))], "found while applying the first level: none set yet");
        assert!(h.calls.iter().any(|c| c.contains("benchmark")));
        // the dynamic level
        assert_eq!(m.get_min_dynamic_particle_priority(), None);
        assert!(m.undefined.is_some(), "read before any level was applied: undefined");
        m.undefined = None;
        assert_eq!(m.find_dynamic_lod_level(10.0), Some(DYNAMIC_GAME_LOD_HIGH));
        assert_eq!(m.find_dynamic_lod_level(30.0), Some(DYNAMIC_GAME_LOD_VERY_HIGH));
        m.display_draw(true, true, 0.5);
        assert_eq!(m.get_dynamic_lod_level(), DYNAMIC_GAME_LOD_LOW);
        assert_eq!(m.get_slow_death_scale(), 0.5);
        assert_eq!(m.get_min_dynamic_particle_priority(), Some(priority::DEBRIS_TRAIL));
        let skips: Vec<bool> = (0..8).map(|_| m.is_particle_skipped()).collect();
        assert_eq!(skips, [true, true, false, true, true, true, false, true]);
        m.display_draw(false, true, 0.5);
        assert_eq!(m.get_dynamic_lod_level(), DYNAMIC_GAME_LOD_VERY_HIGH);
        assert!(!m.is_particle_skipped());
    }

    /// The blocks' errors: a field the table lacks is undefined; a custom preset is undefined; a bad value throws.
    #[test]
    fn the_blocks_errors() {
        let load = |t: &[u8]| {
            let mut m = GameLodManager::new();
            Ini::load(b"a.ini", t, IniLoadType::Overwrite, None, game_lod_blocks(), &mut m)
        };
        assert!(load(b"StaticGameLOD Low\n  Bogus = 1\nEnd\n").expect_err("undefined").is_undefined());
        assert!(load(b"LODPreset = Custom P3 1 XX 1\n").expect_err("undefined").is_undefined());
        assert_eq!(load(b"BenchProfile = Q9 1 1 1 1\n").expect_err("throws").kind, IniErrorKind::InvalidData);
        assert!(load(b"StaticGameLOD Nope\nEnd\n").is_err(), "the fields read as blocks");
        let mut m = GameLodManager::new();
        Ini::load(b"a.ini", b"LODPreset = low P3 1 XX 1\n", IniLoadType::Overwrite, None, game_lod_blocks(), &mut m).expect("reads");
        assert_eq!(m.num_level_presets, [1, 0, 0]);
    }

    #[test]
    fn the_average_frame_rate() {
        let mut a = AverageFps::default();
        a.update(1000, 1000);
        assert_eq!(a.num_samples, 0, "the first interval (from 0) is a spike");
        a.update(1100, 1000);
        a.update(1300, 1000);
        assert_eq!(a.num_samples, 2);
        assert_eq!(a.average_fps, 7.5);
    }
}
