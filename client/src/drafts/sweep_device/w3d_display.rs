//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp
//!
//! Sweep 2 (#213). Drafted: `initAssets`, `init3DScene`, `init2DScene` (empty upstream), `drawDebugStats`, `drawFPSStats`,
//! `drawCurrentDebugDisplay`, `calculateTerrainLOD`, `getLastFrameDrawCalls`, `toggleMovieCapture`, `preloadModelAssets`,
//! `preloadTextureAssets`. Text drawing and timing are the caller's (closures), so the logic can be tested.
//!
//! Excluded (listed in the #213 comment):
//! - `StatDumpClass::dumpStats` (W3DDisplay.cpp:197) is inside `#ifdef DUMP_PERF_STATS`, which GameCommon.h:62-66 defines only
//!   for `_INTERNAL`/`_DEBUG` builds (debug only).
//! - `W3DDisplay::dumpModelAssets` (3186) and `dumpAssetUsage` (3283) are inside `#if defined(_DEBUG) || defined(_INTERNAL)`.
//! Not drafted: `gatherDebugStats` (936-1493: per-frame debug text of ~60 formatted lines from device, particle and logic
//! counters; left to a follow-up).

/// `TerrainLOD` (TerrainVisual.h:173), by value as the port's `common::global_data` has them.
pub const TERRAIN_LOD_MIN: i32 = 1;
pub const TERRAIN_LOD_HALF_CLOUDS: i32 = 3;
pub const TERRAIN_LOD_NO_WATER: i32 = 6;
pub const TERRAIN_LOD_MAX: i32 = 7;
pub const TERRAIN_LOD_AUTOMATIC: i32 = 8;
pub const TERRAIN_LOD_DISABLE: i32 = 9;

/// `W3DDisplay::initAssets` (W3DDisplay.cpp:629): empty upstream.
pub fn init_assets() {}
/// `W3DDisplay::init3DScene` (W3DDisplay.cpp:637): empty upstream.
pub fn init_3d_scene() {}
/// `W3DDisplay::init2DScene` (W3DDisplay.cpp:646): empty upstream.
pub fn init_2d_scene() {}

/// A call to `DisplayString::draw(x, y, textColor, dropColor)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayStringDraw {
    pub index: usize,
    pub x: i32,
    pub y: i32,
    /// `GameMakeColor(255, 255, 255, 255)` / `(0, 0, 0, 255)`.
    pub text_color: u32,
    pub drop_color: u32,
}

/// `GameMakeColor(r, g, b, a)` = `(a << 24) | (r << 16) | (g << 8) | b`.
pub fn game_make_color(r: u32, g: u32, b: u32, a: u32) -> u32 {
    (a << 24) | (r << 16) | (g << 8) | b
}

/// `W3DDisplay::drawDebugStats` (W3DDisplay.cpp:1494): the debug strings drawn in a column from (3, 3), each line `h` high
/// (`getSize`). `lines_of_strings` is `DisplayStringCount`, or `DX8Wrapper::stats.m_debugLinesToShow` when
/// `EXTENDED_STATS` (not defined) is set and not -1; `heights[i]` is string i's height.
pub fn draw_debug_stats(lines_of_strings: usize, heights: &[i32]) -> Vec<DisplayStringDraw> {
    let x = 3;
    let mut y = 3;
    let text_color = game_make_color(255, 255, 255, 255);
    let drop_color = game_make_color(0, 0, 0, 255);
    let mut out = Vec::new();
    for i in 0..lines_of_strings {
        out.push(DisplayStringDraw { index: i, x, y, text_color, drop_color });
        y += heights[i];
    }
    out
}

/// `W3DDisplay::drawFPSStats` (W3DDisplay.cpp:1524): the benchmark string at (3, 20), once.
pub fn draw_fps_stats() -> DisplayStringDraw {
    DisplayStringDraw { index: 0, x: 3, y: 20, text_color: game_make_color(255, 255, 255, 255), drop_color: game_make_color(0, 0, 0, 255) }
}

/// What `drawCurrentDebugDisplay` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugDisplayAction {
    /// `m_debugDisplayCallback == StatDebugDisplay`: `drawDebugStats()`.
    DrawDebugStats,
    /// `m_debugDisplay->reset(); m_debugDisplayCallback(m_debugDisplay, userData)`.
    ResetAndCallCallback,
    /// No display or no callback: nothing.
    Nothing,
}

/// `W3DDisplay::drawCurrentDebugDisplay` (W3DDisplay.cpp:1549).
pub fn draw_current_debug_display(callback_is_stat_debug_display: bool, have_debug_display: bool, have_callback: bool) -> DebugDisplayAction {
    if callback_is_stat_debug_display {
        DebugDisplayAction::DrawDebugStats
    } else if have_debug_display && have_callback {
        DebugDisplayAction::ResetAndCallCallback
    } else {
        DebugDisplayAction::Nothing
    }
}

/// `W3DDisplay::calculateTerrainLOD` (W3DDisplay.cpp:1568): picks the best terrain LOD that renders within
/// `TheGlobalData->m_terrainLODTargetTimeMS`. Starting from AUTOMATIC it tries MAX, then NO_WATER, then HALF_CLOUDS
/// (stopping at DISABLE), timing `NUM_SAMPLES` = 20 frames at each (the first `NUM_TO_DISCARD` = 5 are warm up, and a
/// sample loop ends early when a frame averages over twice the limit); a LOD whose average frame time is under the limit
/// ends the search, and the best LOD so far that met it (`goodLOD`, starting at MIN) is kept. At most 10 tries.
/// `time_one_frame(lod, sample)` renders one frame at that LOD (`adjustTerrainLOD(0)` was applied by the caller on LOD
/// changes) and returns its time in seconds. The `_DEBUG` early return (forces NO_WATER) is not compiled.
/// Returns the chosen LOD to be stored in `m_terrainLOD`.
pub fn calculate_terrain_lod(terrain_lod_target_time_ms: f32, set_lod: &mut impl FnMut(i32), time_one_frame: &mut impl FnMut(i32, i32) -> f32) -> i32 {
    const NUM_SAMPLES: i32 = 20;
    const NUM_TO_DISCARD: i32 = 5;
    let max_time_limit = terrain_lod_target_time_ms / 1000.0;
    let mut good_lod = TERRAIN_LOD_MIN;
    let mut cur_lod = TERRAIN_LOD_AUTOMATIC;
    let mut count = 0;
    loop {
        let mut frame_time = 0.0f32;
        cur_lod = match cur_lod {
            TERRAIN_LOD_AUTOMATIC => TERRAIN_LOD_MAX,
            TERRAIN_LOD_MAX => TERRAIN_LOD_NO_WATER,
            TERRAIN_LOD_HALF_CLOUDS => TERRAIN_LOD_DISABLE,
            TERRAIN_LOD_NO_WATER => TERRAIN_LOD_HALF_CLOUDS,
            _ => TERRAIN_LOD_DISABLE,
        };
        if cur_lod == TERRAIN_LOD_DISABLE {
            break;
        }
        set_lod(cur_lod); // m_terrainLOD = curLOD; drawTerrainOnly(true); adjustTerrainLOD(0)
        let mut i = 0;
        while i < NUM_SAMPLES {
            let time_for_frame = time_one_frame(cur_lod, i);
            if i >= NUM_TO_DISCARD {
                frame_time += time_for_frame;
                if i > NUM_TO_DISCARD + 1 && (time_for_frame / ((i + 1) - NUM_TO_DISCARD) as f32) > 2.0 * max_time_limit {
                    i += 1;
                    break;
                }
            }
            i += 1;
        }
        frame_time /= (i - NUM_TO_DISCARD) as f32;
        count += 1;
        if frame_time < max_time_limit && good_lod < cur_lod {
            good_lod = cur_lod;
        }
        if frame_time < max_time_limit {
            break;
        }
        if count >= 10 {
            break;
        }
    }
    good_lod
}

/// `W3DDisplay::getLastFrameDrawCalls` (W3DDisplay.cpp:1653): `Debug_Statistics::Get_Draw_Calls()`; the renderer's count.
pub fn get_last_frame_draw_calls(draw_calls: i32) -> i32 {
    draw_calls
}

/// The `WW3D::Toggle_Movie_Capture` arguments of `W3DDisplay::toggleMovieCapture` (W3DDisplay.cpp:3123).
pub const MOVIE_CAPTURE_FILE_PREFIX: &str = "Movie";
/// Frames per second of the capture.
pub const MOVIE_CAPTURE_FPS: i32 = 30;

/// `W3DDisplay::preloadModelAssets` (W3DDisplay.cpp:3219): the asset file name loaded through
/// `m_assetManager->Load_3D_Assets` (`"%s.w3d"`); None without an asset manager.
pub fn preload_model_assets(have_asset_manager: bool, model: &str) -> Option<String> {
    if have_asset_manager {
        return Some(format!("{}.w3d", model));
    }
    None
}

/// `W3DDisplay::preloadTextureAssets` (W3DDisplay.cpp:3236): the texture name fetched (`Get_Texture`) and immediately
/// released, so it stays cached; None without an asset manager.
pub fn preload_texture_assets(have_asset_manager: bool, texture: &str) -> Option<String> {
    if have_asset_manager {
        return Some(texture.to_string());
    }
    None
}
