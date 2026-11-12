#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/GameLOD.cpp (the whole file)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp (W3DDisplay::updateAverageFPS, and
//     the LOD manager's calls in W3DDisplay::init and W3DDisplay::draw)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The game's level of detail (`client::game_lod`) against the C++, #160.
//!
//! # Method
//!
//! The C++ side is GameLOD.cpp cut verbatim (see `differential/wrap_game_lod.cpp`), with the REAL INI reader
//! (`INI::load` and its field parsers, wrap_ini.cpp's build) and the REAL GameLOD.h. Each case:
//!
//! - a random `Data\INI\GameLOD.ini` and `Data\INI\GameLODPresets.ini` (all five blocks: the four static and four
//!   dynamic levels by name in any case, unknown names, every field with good and bad values, `LODPreset`s past 32 a
//!   level, `BenchProfile`s past 16, missing tokens, missing `End`s, a stray token; now and then a field the tables lack
//!   or a `LODPreset` of the level `Custom`, both undefined upstream), and sometimes a third file loaded later;
//! - the machine (`testMinimumRequirements`: a video chip, a processor, and a speed, memory and benchmark each known or
//!   unknown, as the Rust client answers), `Options.ini`'s `StaticGameLOD` and `IdealStaticGameLOD`, the custom
//!   level's options, and the global data (`m_forceBenchmark`, `m_enableStaticLOD`, ...);
//! - `init`, then 1 to 14 random operations: `findStaticLODLevel`, `setStaticLODLevel` (-1 .. 3),
//!   `findDynamicLODLevel` (random frame rates, NaN and huge ones among them), `setDynamicLODLevel`,
//!   `getRecommendedTextureReduction`, `getLevelTextureReduction`, `setCurrentTextureReduction`, `isParticleSkipped`
//!   and `isDebrisSkipped` runs, the two name lookups and names, `setReallyLowMHz`, the global data's two switches,
//!   W3DDisplay's frame (`updateAverageFPS` at a random tick, then `draw`'s dynamic LOD) and init's static LOD, and an
//!   INI load.
//!
//! After each operation, its result, the host's calls, and the whole manager (every member, the four static and four
//! dynamic levels, each preset and profile) and the global data are compared as text. A case whose Rust run meets an
//! undefined step (`GameLodManager::undefined`, or an undefined INI read) is cut before that operation and counted; the
//! C++ runs only the defined part.
//!
//! # Predictions (written before the first run, 2026-11-12)
//!
//! 0 mismatches over 20,000 cases; about 2 % of cases cut for an undefined step (the rare unknown field or custom
//! preset, a NaN or huge frame rate, a frame interval of zero). Planted bugs (`L9_PLANT=<id>`), each caught:
//!
//! | id | planted bug | predicted |
//! |---|---|---|
//! | 9541 | the preset match's limit 0.95 | caught (speeds and memories near the limit are generated) |
//! | 9542 | the video chip compared with `>` | caught |
//! | 9543 | an unknown chip presumed a TNT | caught |
//! | 9544 | custom not applied again | caught |
//! | 9545 | a level's own trees requested | caught |
//! | 9546 | the shell map kept on a slow processor | caught |
//! | 9547 | the decals' change ignored for the shadows | caught |
//! | 9548 | the dynamic level at its minimum rate | caught (integral rates are generated) |
//! | 9549 | the particle count tested before its increment | caught |
//! | 9550 | the particle count kept by a new level | caught |
//! | 9551 | the benchmark's memory ignored | caught |
//! | 9552 | the custom tree sway inverted | caught |
//! | 9553 | a preset's speed and chip read in the other order | caught (as a throw or the values) |
//! | 9554 | the low textures not forced without memory | caught |
//! | 9555 | a half-second frame taken as a spike | caught, rarely |
//! | 9556 | the custom level's scaffolds inverted | caught |
//!
//! # Results (2026-11-12, `GAME_LOD_CASES=20000`, the default)
//!
//! **First run:** 20,000 cases, 135,061 operations, **0 mismatches**, but **6,144 cases cut** at an undefined step, not
//! the ~2 % predicted (a miss): 5,689 at `findDynamicLODLevel`. A frame interval of 0 (generated 1 frame in 10) gives
//! `1.0/0` = an infinite rate, which stays in `updateAverageFPS`'s 30-frame history (its statics live across the
//! cases, on both sides), so every later frame with the dynamic LOD on converts infinity to `Int`: undefined. The
//! generator now makes a zero interval 1 frame in 400.
//!
//! **Then** (and after the two generator changes below): 20,000 cases, 160,858 operations, **0 mismatches**; 1,501 cut
//! (1,040 an infinite or NaN rate, 426 an undefined read in the init's INI files: a field the tables lack or a
//! `Custom` preset, 35 the same in a later load).
//!
//! Planted bugs: **16 of 16 caught**, mismatching cases: 9541 4, 9542 4, 9543 2,448, 9544 206, 9545 7,733, 9546 70,
//! 9547 131, 9548 706, 9549 228, 9550 2,150, 9551 3, 9552 8, 9553 4,404, 9554 10, 9555 7,889, 9556 3,441. **Misses
//! against the predictions:** 9541 and 9542 (the preset match's limit and the chip compare) were caught **0 and 1**
//! times at first: the match is reached only while the ideal level is unknown, and with up to 40 presets another one
//! decides it. The generator now gives the ideal level 1 case in 3, draws half the presets and the machine from a few
//! near-limit values (940 against 1000 MHz, 241 against 256 MB, TNT2 / GF3), and mostly writes 8 presets; they are now
//! caught 4 times each, still rarely. 9555 (the half-second frame) was predicted rare and is caught often: half a second
//! is generated 1 frame in 10.

use rts_lockstep_engine::client::game_lod::*;
use rts_lockstep_engine::common::global_data::GlobalData;
use rts_lockstep_engine::common::ini::{Ini, IniError, IniLoadType, IniResult, Thrown};
use rts_lockstep_engine::differential as cpp;
use std::collections::BTreeMap;
use std::ffi::CString;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[self.below(xs.len() as u64) as usize]
    }
    fn chance(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }
}

fn hex(b: &[u8]) -> String {
    if b.is_empty() {
        return "-".into();
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

const STATIC_NAMES: [&str; 9] = ["Low", "Medium", "High", "Custom", "low", "HIGH", "mEdIuM", "VeryHigh", "Bogus"];
const DYNAMIC_NAMES: [&str; 8] = ["Low", "Medium", "High", "VeryHigh", "veryhigh", "LOW", "Custom", "Bogus"];
const STATIC_FIELDS: [(&str, bool); 19] = [
    ("MinimumFPS", false),
    ("MinimumProcessorFps", false),
    ("SampleCount2D", false),
    ("SampleCount3D", false),
    ("StreamCount", false),
    ("MaxParticleCount", false),
    ("UseShadowVolumes", true),
    ("UseShadowDecals", true),
    ("UseCloudMap", true),
    ("UseLightMap", true),
    ("ShowSoftWaterEdge", true),
    ("MaxTankTrackEdges", false),
    ("MaxTankTrackOpaqueEdges", false),
    ("MaxTankTrackFadeDelay", false),
    ("UseBuildupScaffolds", true),
    ("UseTreeSway", true),
    ("UseEmissiveNightMaterials", true),
    ("UseHeatEffects", true),
    ("TextureReductionFactor", false),
];
const PRIORITIES: [&str; 15] = [
    "NONE", "WEAPON_EXPLOSION", "SCORCHMARK", "DUST_TRAIL", "BUILDUP", "DEBRIS_TRAIL", "UNIT_DAMAGE_FX", "DEATH_EXPLOSION", "SEMI_CONSTANT", "CONSTANT", "WEAPON_TRAIL", "AREA_EFFECT", "CRITICAL",
    "ALWAYS_RENDER", "critical",
];
const CPUS: [&str; 6] = ["XX", "P3", "P4", "K7", "p4", "Q9"];
const VIDEOS: [&str; 18] = ["XX", "V2", "V3", "V4", "V5", "TNT", "TNT2", "GF2", "R100", "PS11", "GF3", "GF4", "PS14", "R200", "PS20", "R300", "tnt2", "GF9"];

fn int_tok(r: &mut Rng) -> String {
    match r.below(12) {
        0 => "abc".into(),
        1 => (r.below(5) as i64 - 2).to_string(),
        2 => [0, 1, 2, 3, 7, 15, 255, -1][r.below(8) as usize].to_string(),
        3 => format!("{}", r.below(1 << 31) as i64 - (1 << 30)),
        _ => r.below(4000).to_string(),
    }
}
fn bool_tok(r: &mut Rng) -> String {
    r.pick(&["Yes", "No", "yes", "NO", "1", "maybe"]).to_string()
}
fn real_tok(r: &mut Rng) -> String {
    match r.below(10) {
        0 => "x".into(),
        1 => "0".into(),
        2 => format!("-{}.{}", r.below(3), r.below(100)),
        _ => format!("{}.{:02}", r.below(4), r.below(100)),
    }
}
fn mhz_tok(r: &mut Rng) -> String {
    if r.chance(15) {
        return int_tok(r);
    }
    [300, 450, 500, 733, 800, 940, 1000, 1063, 1400, 1500, 1880, 2000, 2400, 3000][r.below(14) as usize].to_string()
}
fn mem_tok(r: &mut Rng) -> String {
    if r.chance(15) {
        return int_tok(r);
    }
    [64, 128, 240, 241, 256, 384, 512, 1024][r.below(8) as usize].to_string()
}

/// A preset line: `LODPreset = level cpu mhz video memory`, now and then cut short.
fn preset_line(r: &mut Rng) -> String {
    let level = if r.chance(150) { "Custom" } else { r.pick(&["Low", "Medium", "High", "low", "high", "Bogus"]) };
    // (half the presets on a few values, which the machine's near-limit values meet: the match's edges are reached)
    let near = r.chance(2);
    let toks = if near {
        [r.pick(&["P3", "P4"]).to_string(), r.pick(&["1000", "2000"]).to_string(), r.pick(&["TNT2", "GF3"]).to_string(), r.pick(&["256", "512"]).to_string()]
    } else {
        [r.pick(&CPUS).to_string(), mhz_tok(r), r.pick(&VIDEOS).to_string(), mem_tok(r)]
    };
    let n = if r.chance(20) { r.below(4) as usize } else { 4 };
    format!("LODPreset = {level} {}\n", toks[..n].join(" "))
}
fn bench_line(r: &mut Rng) -> String {
    let toks = [r.pick(&CPUS).to_string(), mhz_tok(r), real_tok(r), real_tok(r), real_tok(r)];
    let n = if r.chance(20) { r.below(5) as usize } else { 5 };
    format!("BenchProfile = {}\n", toks[..n].join(" "))
}

fn lod_file(r: &mut Rng, presets: bool) -> Vec<u8> {
    let mut t = String::from("; synthetic\n");
    // (the presets file: past 32 presets a level now and then, but mostly a few, so that one preset decides the match)
    let many = r.chance(4);
    let blocks = r.below(if presets { if many { 80 } else { 8 } } else { 12 });
    for _ in 0..blocks {
        let k = if presets { if r.chance(6) { r.below(6) } else { 2 + r.below(2) } } else { r.below(12) };
        match k {
            0 | 1 | 6 | 7 | 8 => {
                t += &format!("StaticGameLOD = {}\n", r.pick(&STATIC_NAMES));
                for _ in 0..r.below(8) {
                    let (f, b) = STATIC_FIELDS[r.below(19) as usize];
                    let v = if b { bool_tok(r) } else { int_tok(r) };
                    t += &format!("  {f} = {v}\n");
                }
                if r.chance(200) {
                    t += "  Bogus = 1\n";
                }
                if !r.chance(40) {
                    t += "End\n";
                }
            }
            4 | 5 | 9 => {
                t += &format!("DynamicGameLOD = {}\n", r.pick(&DYNAMIC_NAMES));
                for _ in 0..r.below(6) {
                    t += &match r.below(6) {
                        0 => format!("  MinimumFPS = {}\n", int_tok(r)),
                        1 => format!("  ParticleSkipMask = {}\n", int_tok(r)),
                        2 => format!("  DebrisSkipMask = {}\n", int_tok(r)),
                        3 => format!("  SlowDeathScale = {}\n", real_tok(r)),
                        4 => format!("  MinParticlePriority = {}\n", r.pick(&PRIORITIES)),
                        _ => format!("  MinParticleSkipPriority = {}\n", if r.chance(10) { "BOGUS" } else { r.pick(&PRIORITIES) }),
                    };
                }
                if r.chance(200) {
                    t += "  ParticleSkip = 1\n";
                }
                if !r.chance(40) {
                    t += "End\n";
                }
            }
            2 => t += &preset_line(r),
            3 => t += &bench_line(r),
            10 => t += &format!("ReallyLowMHz = {}\n", if r.chance(10) { "x".to_string() } else { mhz_tok(r) }),
            _ => {
                if r.chance(4) {
                    t += "Stray = 1\n";
                }
            }
        }
    }
    // a few good dynamic levels, so that the frame rate picks among them
    if !presets && r.below(3) != 0 {
        for (i, n) in DYNAMIC_GAME_LOD_NAMES.iter().enumerate() {
            t += &format!("DynamicGameLOD = {n}\n  MinimumFPS = {}\n  ParticleSkipMask = {}\n  DebrisSkipMask = {}\n  SlowDeathScale = 0.{}\n  MinParticlePriority = {}\nEnd\n", 5 * i + r.below(10) as usize, r.below(16), r.below(8), r.below(10), r.pick(&PRIORITIES[1..13]));
        }
    }
    t.into_bytes()
}

// ---------------------------------------------------------------- the Rust side's host

#[derive(Default)]
struct Machine {
    video: i32,
    cpu: i32,
    freq: Option<i32>,
    ram: Option<i32>,
    bench: Option<(f32, f32, f32)>,
    custom: [i32; 13],
    options: BTreeMap<Vec<u8>, Vec<u8>>,
    log: String,
}

impl GameLodHost for Machine {
    fn video_chip_type(&mut self) -> i32 {
        self.log += "host video\n";
        self.video
    }
    fn cpu_freq_ram(&mut self) -> (i32, Option<i32>, Option<i32>) {
        self.log += "host cpu\n";
        (self.cpu, self.freq, self.ram)
    }
    fn benchmark(&mut self) -> Option<(f32, f32, f32)> {
        self.log += "host bench\n";
        self.bench
    }
    fn write_benchmark_file(&mut self, text: &[u8]) {
        self.log += &format!("host benchfile {}\n", String::from_utf8_lossy(text));
    }
    fn adjust_lod(&mut self, adj: i32) {
        self.log += &format!("host adjustLOD {adj}\n");
    }
    fn reallocate_shadows(&mut self) {
        self.log += "host shadows\n";
    }
    fn set_shore_line_detail(&mut self) {
        self.log += "host shore\n";
    }
    fn set_terrain_tracks_detail(&mut self) {
        self.log += "host tracks\n";
    }
    fn save_lod_preferences(&mut self, ideal: &[u8], static_level: Option<&[u8]>) {
        self.options.insert(b"IdealStaticGameLOD".to_vec(), ideal.to_vec());
        if let Some(s) = static_level {
            self.options.insert(b"StaticGameLOD".to_vec(), s.to_vec());
        }
        self.log += "host write";
        for (k, v) in &self.options {
            self.log += &format!(" {}={}", String::from_utf8_lossy(k), String::from_utf8_lossy(v));
        }
        self.log += "\n";
    }
}

struct Prefs {
    options: BTreeMap<Vec<u8>, Vec<u8>>,
    custom: [i32; 13],
}

impl LodOptionPrefs for Prefs {
    fn static_game_lod(&self) -> Option<Vec<u8>> {
        self.options.get(&b"StaticGameLOD"[..]).cloned()
    }
    fn ideal_static_game_lod(&self) -> Option<Vec<u8>> {
        self.options.get(&b"IdealStaticGameLOD"[..]).cloned()
    }
    fn custom(&mut self, _: &GlobalData) -> LodCustomPrefs {
        let c = &self.custom;
        LodCustomPrefs {
            texture_reduction: c[0],
            shadows_3d: c[1] != 0,
            shadows_2d: c[2] != 0,
            building_occlusion: c[3] != 0,
            particle_cap: c[4],
            dynamic_lod: c[5] != 0,
            fps_limit: c[6] != 0,
            lightmap: c[7] != 0,
            cloud_shadows: c[8] != 0,
            smooth_water: c[9] != 0,
            heat_effects: c[10] != 0,
            extra_animations_disabled: c[11] != 0,
            trees: c[12] != 0,
        }
    }
}

fn b(x: bool) -> i32 {
    i32::from(x)
}

fn dump(m: &GameLodManager, g: &GlobalData) -> String {
    let p = |v: Option<i32>| v.map_or(" ?".to_string(), |v| format!(" {v}"));
    let mut o = format!(
        "mgr {} {} {} {} {} {} {:08x}{}{} {} {} {} {} {} {} {} {} {} {} {} {} {}",
        m.current_static_lod,
        m.current_dynamic_lod,
        m.num_particle_generations,
        m.dynamic_particle_skip_mask,
        m.num_debris_generations,
        m.dynamic_debris_skip_mask,
        m.slow_death_scale.to_bits(),
        p(m.min_dynamic_particle_priority),
        p(m.min_dynamic_particle_skip_priority),
        b(m.video_passed),
        b(m.cpu_passed),
        b(m.mem_passed),
        m.num_level_presets[0],
        m.num_level_presets[1],
        m.num_level_presets[2],
        m.num_bench_profiles,
        m.ideal_detail_level,
        m.video_chip_type,
        m.cpu_type,
        m.num_ram,
        m.cpu_freq,
        m.current_texture_reduction
    );
    o += &format!(
        " {:08x} {:08x} {:08x} {:08x} {}\n",
        m.int_bench_index.to_bits(),
        m.float_bench_index.to_bits(),
        m.mem_bench_index.to_bits(),
        m.composite_bench_index.to_bits(),
        m.really_low_mhz
    );
    for (i, l) in m.static_game_lod_info.iter().enumerate() {
        o += &format!(
            "static {i} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}\n",
            l.min_fps,
            l.min_processor_fps,
            l.sample_count_2d,
            l.sample_count_3d,
            l.stream_count,
            l.max_particle_count,
            b(l.use_shadow_volumes),
            b(l.use_shadow_decals),
            b(l.use_cloud_map),
            b(l.use_light_map),
            b(l.show_soft_water_edge),
            l.max_tank_track_edges,
            l.max_tank_track_opaque_edges,
            l.max_tank_track_fade_delay,
            b(l.use_buildup_scaffolds),
            b(l.use_tree_sway),
            b(l.use_emissive_night_materials),
            b(l.use_heat_effects),
            l.texture_reduction,
            b(l.use_fps_limit),
            b(l.enable_dynamic_lod),
            b(l.use_trees)
        );
    }
    for (i, l) in m.dynamic_game_lod_info.iter().enumerate() {
        o += &format!(
            "dynamic {i} {} {} {} {:08x} {} {}\n",
            l.min_fps,
            l.dynamic_particle_skip_mask,
            l.dynamic_debris_skip_mask,
            l.slow_death_scale.to_bits(),
            l.min_dynamic_particle_priority,
            l.min_dynamic_particle_skip_priority
        );
    }
    for i in 0..3 {
        for j in 0..(m.num_level_presets[i] as usize).min(MAX_LOD_PRESETS_PER_LEVEL) {
            let q = &m.lod_presets[i][j];
            o += &format!("preset {i} {j} {} {} {} {}\n", q.cpu_type, q.mhz, q.video_type, q.memory);
        }
    }
    for k in 0..(m.num_bench_profiles as usize).min(MAX_BENCH_PROFILES) {
        let q = &m.bench_profiles[k];
        o += &format!("bench {k} {} {} {:08x} {:08x} {:08x}\n", q.cpu_type, q.mhz, q.int_bench_index.to_bits(), q.float_bench_index.to_bits(), q.mem_bench_index.to_bits());
    }
    o += &format!(
        "gd {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}\n",
        b(g.force_benchmark),
        b(g.enable_static_lod),
        b(g.enable_dynamic_lod),
        g.texture_reduction_factor,
        g.max_particle_count,
        g.max_tank_track_edges,
        g.max_tank_track_opaque_edges,
        g.max_tank_track_fade_delay,
        b(g.use_shadow_volumes),
        b(g.use_shadow_decals),
        b(g.enable_behind_building_markers),
        b(g.use_fps_limit),
        b(g.use_light_map),
        b(g.use_cloud_map),
        b(g.show_soft_water_edge),
        b(g.use_heat_effects),
        b(g.use_draw_module_lod),
        b(g.use_tree_sway),
        b(g.use_trees),
        b(g.shell_map_on)
    );
    o
}

/// The Rust result of an INI read, as the wrapper prints a throw; `None` for an undefined read.
fn outcome(r: IniResult<()>) -> Option<String> {
    match r {
        Ok(()) => Some("ok\n".into()),
        Err(e) => Some(match e.thrown()? {
            Thrown::Exception(m) => format!("throw 1 {}\n", String::from_utf8_lossy(&m)),
            Thrown::Status(c) => format!("throw 2 {c}\n"),
            Thrown::ErrorCode(c) => format!("throw 3 {c}\n"),
            _ => "throw 5\n".into(),
        }),
    }
}

fn cpp_run(text: &str) -> String {
    let c = CString::new(text).expect("no NUL");
    let mut out = vec![0u8; 1 << 22];
    let n = unsafe { cpp::ref_game_lod_run(c.as_ptr(), out.as_mut_ptr() as *mut _, out.len() as i32) };
    assert!((n as usize) <= out.len(), "the dump overflowed");
    String::from_utf8_lossy(&out[..n as usize]).into_owned()
}

/// The clock of the frame operations (`getPerformanceCounter`), shared by every case as updateAverageFPS's statics are.
struct Clock {
    now: i64,
    fps: AverageFps,
}

fn case(r: &mut Rng, clock: &mut Clock) -> (String, String, Option<String>) {
    let more_presets = r.chance(2);
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("Data\\INI\\GameLOD.ini", lod_file(r, false)),
        ("Data\\INI\\GameLODPresets.ini", lod_file(r, true)),
        ("Data\\INI\\More.ini", lod_file(r, more_presets)),
    ];
    let missing = r.chance(60).then(|| r.below(2) as usize);
    let mut head = String::new();
    for (i, (n, d)) in files.iter().enumerate() {
        if missing != Some(i) {
            head += &format!("file {n} {}\n", hex(d));
        }
    }
    fn bench_index(r: &mut Rng) -> f32 {
        if r.chance(10) {
            0.0
        } else {
            (r.below(400) as f32) / 100.0
        }
    }
    let near = r.chance(2);
    let video = if near { [5, 6, 9, 10][r.below(4) as usize] } else if r.chance(3) { 0 } else { r.below(17) as i32 };
    let cpu = if near { 1 + r.below(2) as i32 } else { r.below(4) as i32 };
    let freq = if r.chance(3) { None } else if near { Some([940, 1880, 1000, 2000, 949, 1890][r.below(6) as usize]) } else { Some(mhz_tok(r).parse().unwrap_or(1000)) };
    let ram_mb = if near { [241, 242, 256, 482, 484, 512][r.below(6) as usize] } else { [64, 128, 240, 241, 256, 384, 512, 1024][r.below(8) as usize] };
    let ram = if r.chance(3) { None } else { Some(ram_mb * 1024 * 1024 + r.below(3) as i32 - 1) };
    let bench = if r.chance(3) { None } else { Some((bench_index(r), bench_index(r), bench_index(r))) };
    let mut mc = Machine { video, cpu, freq, ram, bench, ..Default::default() };
    // (the ideal level given in 1 case of 3: without it the presets are matched, which the plants on the match need)
    for (k, odds) in [(b"StaticGameLOD".as_slice(), 2), (b"IdealStaticGameLOD", 3)] {
        if r.below(odds) == 0 || (odds == 2 && r.chance(2)) {
            mc.options.insert(k.to_vec(), r.pick(&STATIC_NAMES).as_bytes().to_vec());
        }
    }
    for (k, v) in &mc.options {
        head += &format!("opt {} {}\n", String::from_utf8_lossy(k), hex(v));
    }
    let (bi, bf, bm) = mc.bench.unwrap_or((0.0, 0.0, 0.0));
    head += &format!(
        "machine {} {} {} {} {} {} {} {:08x} {:08x} {:08x}\n",
        mc.video,
        mc.cpu,
        b(mc.freq.is_some()),
        mc.freq.unwrap_or(0),
        b(mc.ram.is_some()),
        mc.ram.unwrap_or(0),
        b(mc.bench.is_some()),
        bi.to_bits(),
        bf.to_bits(),
        bm.to_bits()
    );
    mc.custom = [r.below(5) as i32 - 2, 0, 0, 0, 50 + r.below(5000) as i32, 0, 0, 0, 0, 0, 0, 0, 0];
    for i in (1..4).chain(5..13) {
        mc.custom[i] = r.below(2) as i32;
    }
    head += &format!("custom {}\n", mc.custom.map(|c| c.to_string()).join(" "));
    let mut g = GlobalData::new();
    g.force_benchmark = r.chance(6);
    g.enable_static_lod = !r.chance(8);
    g.enable_dynamic_lod = r.chance(2);
    g.texture_reduction_factor = r.below(4) as i32 - 1;
    g.max_particle_count = r.below(5000) as i32;
    g.max_tank_track_edges = r.below(200) as i32;
    g.max_tank_track_opaque_edges = r.below(50) as i32;
    g.max_tank_track_fade_delay = r.below(500000) as i32;
    for f in [
        &mut g.use_shadow_volumes,
        &mut g.use_shadow_decals,
        &mut g.enable_behind_building_markers,
        &mut g.use_fps_limit,
        &mut g.use_light_map,
        &mut g.use_cloud_map,
        &mut g.show_soft_water_edge,
        &mut g.use_heat_effects,
        &mut g.use_draw_module_lod,
        &mut g.use_tree_sway,
        &mut g.use_trees,
        &mut g.shell_map_on,
    ] {
        *f = r.chance(2);
    }
    head += &dump(&GameLodManager::new(), &g).lines().last().expect("gd").replacen("gd ", "gd ", 1);
    head += "\n";
    // the operations
    let mut ops = vec!["init".to_string()];
    for _ in 0..1 + r.below(14) {
        ops.push(match r.below(18) {
            0 => "find".into(),
            1 | 2 => format!("set {}", r.below(5) as i32 - 1),
            3 => {
                let f: f32 = match r.below(8) {
                    0 => f32::from_bits(r.next() as u32),
                    1 => r.below(40) as f32,
                    _ => r.below(4000) as f32 / 100.0,
                };
                format!("dfind {:08x}", f.to_bits())
            }
            4 => format!("dset {}", r.below(5) as i32 - 1),
            5 => "rec".into(),
            6 => format!("ltr {}", r.below(4)),
            7 => format!("ctr {}", r.below(4) as i32 - 1),
            8 => format!("pskip {}", 1 + r.below(20)),
            9 => format!("dskip {}", 1 + r.below(20)),
            10 => format!("sidx {}", hex(r.pick(&STATIC_NAMES).as_bytes())),
            11 => format!("didx {}", hex(r.pick(&DYNAMIC_NAMES).as_bytes())),
            12 => format!("{} {}", r.pick(&["sname", "dname"]), r.below(4)),
            13 => format!("rlow {}", mhz_tok(r).parse::<i32>().unwrap_or(400)),
            14 => format!("flags {} {}", b(!r.chance(5)), r.below(2)),
            15 | 16 => {
                let freq = [1000i64, 3_579_545, 60][r.below(3) as usize];
                // (an interval of 0 gives an infinite rate, which stays in the 30-frame history: rare, so that it does not
                // cut most cases after it)
                let dt = match r.below(10) {
                    0 => freq / 2,
                    1 => freq,
                    2 if r.chance(40) => 0,
                    _ => 1 + r.below((freq / 4) as u64) as i64,
                };
                clock.now += dt;
                format!("frame {} {freq} {}", clock.now, r.below(4).min(1))
            }
            _ => {
                if r.chance(2) {
                    "dinit".into()
                } else {
                    "load Data\\INI\\More.ini".into()
                }
            }
        });
    }
    // the Rust run, cut at an undefined step
    let mut m = GameLodManager::new();
    let mut o = String::new();
    let mut ran = 0;
    let mut undefined: Option<String> = None;
    let file_data = |p: &[u8]| -> Option<Vec<u8>> { files.iter().enumerate().find(|(i, (n, _))| n.as_bytes() == p && missing != Some(*i)).map(|(_, (_, d))| d.clone()) };
    for op in &ops {
        let fps_before = clock.fps.clone();
        let mut line = format!("> {op}\n");
        let mut w = op.split(' ');
        let cmd = w.next().expect("op");
        let arg = w.next().unwrap_or("");
        let num = || arg.parse::<i32>().unwrap_or(0);
        let res: Option<String> = match cmd {
            "init" => {
                let mut prefs = Prefs { options: mc.options.clone(), custom: mc.custom };
                outcome(m.init(&mut mc, &mut g, &mut prefs, file_data))
            }
            "load" => {
                let data = file_data(arg.as_bytes());
                match data {
                    Some(d) => outcome(Ini::load(arg.as_bytes(), &d, IniLoadType::Overwrite, None, game_lod_blocks(), &mut m)),
                    None => outcome(Err(IniError::new(rts_lockstep_engine::common::ini::IniErrorKind::CantOpenFile))),
                }
            }
            "find" => Some(format!("= {}\n", m.find_static_lod_level(&mut mc))),
            "set" => Some(format!("= {}\n", b(m.set_static_lod_level(&mut mc, &mut g, num())))),
            "dfind" => m.find_dynamic_lod_level(f32::from_bits(u32::from_str_radix(arg, 16).expect("hex"))).map(|l| format!("= {l}\n")),
            "dset" => Some(format!("= {}\n", b(m.set_dynamic_lod_level(num())))),
            "rec" => Some(format!("= {}\n", m.get_recommended_texture_reduction(&mut mc))),
            "ltr" => m.get_level_texture_reduction(num()).map(|v| format!("= {v}\n")),
            "ctr" => {
                m.set_current_texture_reduction(num());
                Some(format!("= {}\n", m.get_current_texture_reduction()))
            }
            "pskip" | "dskip" => {
                let mut s = "=".to_string();
                for _ in 0..num() {
                    let v = if cmd == "pskip" { m.is_particle_skipped() } else { m.is_debris_skipped() };
                    s += &format!(" {}", b(v));
                }
                Some(s + "\n")
            }
            "sidx" | "didx" => {
                let name: Vec<u8> = (0..arg.len() / 2).map(|i| u8::from_str_radix(&arg[2 * i..2 * i + 2], 16).expect("hex")).collect();
                Some(format!("= {}\n", if cmd == "sidx" { m.get_static_game_lod_index(&name) } else { m.get_dynamic_game_lod_index(&name) }))
            }
            "sname" => m.get_static_game_lod_level_name(num()).map(|n| format!("= {n}\n")),
            "dname" => m.get_dynamic_game_lod_level_name(num()).map(|n| format!("= {n}\n")),
            "rlow" => {
                m.set_really_low_mhz(num());
                Some(format!("= {} {}\n", b(m.is_really_low_mhz()), b(m.did_mem_pass())))
            }
            "flags" => {
                g.enable_static_lod = arg == "1";
                g.enable_dynamic_lod = w.next() == Some("1");
                Some("ok\n".into())
            }
            "frame" => {
                let freq: i64 = w.next().expect("freq").parse().expect("freq");
                let show = w.next() == Some("1");
                clock.fps.update(arg.parse().expect("time"), freq);
                let avg = clock.fps.average_fps;
                m.display_draw(g.enable_dynamic_lod, show, avg);
                Some(format!("= {:08x}\n", avg.to_bits()))
            }
            "dinit" => {
                m.display_init(&mut mc, &mut g);
                Some("ok\n".into())
            }
            _ => unreachable!(),
        };
        let res = match res {
            Some(r) if m.undefined.is_none() => r,
            _ => {
                clock.fps = fps_before;
                undefined = Some(m.undefined.map_or_else(|| format!("{cmd}: an undefined INI read"), |u| u.split(':').next().unwrap_or(u).to_string()));
                break;
            }
        };
        line += &std::mem::take(&mut mc.log);
        line += &res;
        // the wrapper logs the host's calls as they happen, the result after
        o += &line;
        o += &dump(&m, &g);
        ran += 1;
    }
    let text = head + &ops[..ran].iter().map(|o| format!("{o}\n")).collect::<String>();
    (text, o, undefined)
}

/// The C++ prints each operation's line, then the host's calls and the result in the order they came; the Rust test
/// writes the calls before the result. Both are put in that order before they are compared.
fn normalise(s: &str) -> String {
    let mut out = String::new();
    let mut calls = String::new();
    for l in s.lines() {
        if l.starts_with("host ") {
            calls += l;
            calls += "\n";
        } else if l.starts_with("= ") || l == "=" || l == "ok" || l.starts_with("throw ") {
            out += &calls;
            calls.clear();
            out += l;
            out += "\n";
        } else {
            out += &calls;
            calls.clear();
            out += l;
            out += "\n";
        }
    }
    out + &calls
}

#[test]
fn the_game_lod_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("GAME_LOD_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut r = Rng(0x4761_6d65_4c4f_4431);
    let mut clock = Clock { now: 1 << 40, fps: AverageFps::default() };
    let (mut mismatches, mut undefined, mut ops) = (0usize, 0usize, 0usize);
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for n in 0..cases {
        let (text, rust, cut) = case(&mut r, &mut clock);
        if let Some(k) = cut {
            undefined += 1;
            *kinds.entry(k).or_insert(0usize) += 1;
        }
        ops += text.lines().filter(|l| !["file", "opt", "machine", "custom", "gd"].iter().any(|p| l.starts_with(p))).count();
        let c = cpp_run(&text);
        if normalise(&c) != normalise(&rust) {
            mismatches += 1;
            if mismatches <= 2 {
                let (a, b) = (normalise(&rust), normalise(&c));
                let at = a.lines().zip(b.lines()).position(|(x, y)| x != y).unwrap_or(0);
                println!("case {n}:\n{text}\n  first difference at line {at}:\n  rust: {:?}\n  c++:  {:?}", a.lines().nth(at), b.lines().nth(at));
            }
        }
    }
    println!("#160 game LOD: {cases} cases, {ops} operations; {undefined} cut at an undefined step ({kinds:?}); mismatches: {mismatches}");
    if planted {
        println!("MEASURED mismatches {mismatches}");
    }
    assert_eq!(mismatches, 0);
}
