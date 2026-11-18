//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
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
//! Second pass (#213): `gatherDebugStats` (936-1493) is drafted at the bottom. It is NOT debug-only: its caller
//! (W3DDisplay.cpp:1717) runs it whenever `m_debugDisplayCallback == StatDebugDisplay`, which `W3DDisplay::init` (line 833)
//! installs when `TheGlobalData->m_displayDebug` is set, in every build. Only its inner `#if defined(_DEBUG) ||
//! defined(_INTERNAL)` arms (cumulative FPS, "DEBUG app", `pickDrawable`, render cost) and the `EXTENDED_STATS` block
//! (macro not defined) are left out; the `#else` arms, which Release compiles, are translated.

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

// ---------------------------------------------------------------------------------------------------------------------
// gatherDebugStats
// ---------------------------------------------------------------------------------------------------------------------

/// The `W3DDisplay` string slots, in `enum` order (W3DDisplay.h:174-194). `drawDebugStats` draws them in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugString {
    Fps = 0,
    Frame,
    Polygons,
    Vertices,
    VideoRam,
    DebugInfo,
    KeyMouseStates,
    MousePosition,
    Particles,
    Objects,
    NetIncoming,
    NetOutgoing,
    NetStats,
    NetFpsAverages,
    SelectedInfo,
    TerrainStats,
}

/// `DisplayStringCount`.
pub const DISPLAY_STRING_COUNT: usize = 16;

/// The function-local statics of `gatherDebugStats` (`s_framesRenderedSinceLastUpdate`, `s_lastUpdateTime64`,
/// `s_timeSinceLastUpdateInSecs`, `s_drawCallsSinceLastUpdate`, `s_sortedPolysSinceLastUpdate`), all initially 0.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GatherDebugStatsState {
    pub frames_rendered_since_last_update: u32,
    pub last_update_time64: i64,
    pub time_since_last_update_in_secs: f64,
    pub draw_calls_since_last_update: i32,
    pub sorted_polys_since_last_update: i32,
}

/// A selected / moused-over `Drawable` with what the "Select Info" text reads from it.
#[derive(Clone, Debug, Default)]
pub struct DebugDrawableInfo {
    /// `draw->getTemplate()->getName()`.
    pub template_name: String,
    /// `obj->getName()` when the drawable has an object (None: no object; Some("") empty name -> "No-Name").
    pub object_name: Option<String>,
    /// `draw->getPosition()`.
    pub position: (f32, f32, f32),
    /// `physics ? physics->getTurning() : TURN_NONE`, as the enum value.
    pub turn_type: i32,
    /// `draw->getLocoInfo()`: (accelerationPitch, accelerationPitchRate, accelerationRoll, accelerationRollRate).
    pub loco_info: Option<(f32, f32, f32, f32)>,
    /// `ModelConditionFlags::getBitNames()[i]` for each set bit, in bit order.
    pub model_condition_names: Vec<String>,
    /// For the moused-over line: `object->getTemplate()->getName()` and `object->getID()`, when it has an object.
    pub object_for_mouse_over: Option<(String, u32)>,
}

/// The network numbers `gatherDebugStats` reads from `TheNetwork`.
#[derive(Clone, Debug, Default)]
pub struct DebugNetworkStats {
    pub incoming_bytes_per_second: f32,
    pub incoming_packets_per_second: f32,
    pub outgoing_bytes_per_second: f32,
    pub outgoing_packets_per_second: f32,
    pub run_ahead: i32,
    pub frame_rate: i32,
    pub packet_arrival_cushion: i32,
    /// `(getPlayerName(i), getSlotAverageFPS(i))` for each player.
    pub players: Vec<(String, i32)>,
}

/// Everything `gatherDebugStats` reads from the engine's singletons for one call.
#[derive(Clone, Debug, Default)]
pub struct GatherDebugStatsInputs {
    /// `getPerformanceCounterFrequency()` / `getPerformanceCounter()`.
    pub freq64: i64,
    pub time64: i64,
    /// `Debug_Statistics::Get_Draw_Calls / Get_Sorting_Polygons / Get_DX8_Skin_Renders / Get_DX8_Polygons /
    /// Get_DX8_Vertices / Get_Record_Texture_Size`.
    pub draw_calls: i32,
    pub sorting_polygons: i32,
    pub skin_renders: i32,
    pub polygons: i32,
    pub vertices: i32,
    pub record_texture_size: i32,
    /// `TheGlobalData->m_constantDebugUpdate`, `m_useFpsLimit`, `m_framesPerSecondLimit`.
    pub constant_debug_update: bool,
    pub use_fps_limit: bool,
    pub frames_per_second_limit: i32,
    /// `registry.Get_Int("LoadDebugRuntime", 0) == 1` for `Software\\Microsoft\\Direct3d` (Windows registry, supplied).
    pub debug_d3d: bool,
    /// `TheGameLogic->getFrame()`, `getObjectCount()`, `TheGameClient->getRenderedObjectCount()`.
    pub game_logic_frame: u32,
    pub object_count: u32,
    pub rendered_object_count: u32,
    /// `TheParticleSystemManager->getParticleCount()` / `getOnScreenParticleCount()`.
    pub particle_count: i32,
    pub on_screen_particle_count: i32,
    /// `TheTerrainRenderObject->getNumExtraBlendTiles(TRUE/FALSE)` and `getNumShoreLineTiles(TRUE/FALSE)`.
    pub extra_blend_tiles: (i32, i32),
    pub shore_line_tiles: (i32, i32),
    /// `TheTacticalView` getters: position, zoom, pitch, FX pitch, angle, FOV, terrain height under camera, current height
    /// above ground.
    pub cam_pos: (f32, f32, f32),
    pub zoom: f32,
    pub pitch: f32,
    pub fx_pitch: f32,
    pub angle: f32,
    pub fov: f32,
    pub terrain_height: f32,
    pub actual_height_above_ground: f32,
    /// `TheKeyboard->isShift/isCtrl/isAlt`, and `getModifierFlags()` bits KEY_STATE_L/R SHIFT/CONTROL/ALT.
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub l_shift: bool,
    pub r_shift: bool,
    pub l_control: bool,
    pub r_control: bool,
    pub l_alt: bool,
    pub r_alt: bool,
    /// `TheMouse->getMouseStatus()`: left/middle/right state and screen position.
    pub left_state: bool,
    pub middle_state: bool,
    pub right_state: bool,
    pub mouse_pos: (i32, i32),
    /// `TheTacticalView->screenToTerrain(&mouseIO->pos, &worldPos)`.
    pub mouse_world_pos: (f32, f32, f32),
    /// `TheGameClient->findDrawableByID(TheInGameUI->getMousedOverDrawableID())` (the `#else` arm).
    pub moused_over_drawable: Option<DebugDrawableInfo>,
    /// `TheInGameUI->getSelectCount()` and `getFirstSelectedDrawable()`.
    pub select_count: i32,
    pub first_selected_drawable: Option<DebugDrawableInfo>,
    /// `TheNetwork` (None when NULL).
    pub network: Option<DebugNetworkStats>,
    /// `TheGlobalLanguageData->m_nativeDebugDisplay` (name, size, bold) when it has a non-empty name.
    pub native_debug_font: Option<(String, i32, bool)>,
}

/// What one `gatherDebugStats` call produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GatherDebugStatsOutput {
    /// `setText` calls on `m_displayStrings[..]`, in the order upstream makes them.
    pub texts: Vec<(DebugString, String)>,
    /// `m_benchmarkDisplayString->setText(fpsString)` ("FPS: %.2f").
    pub benchmark_text: Option<String>,
    /// When the display strings were not yet allocated: the font they get (`getFont(name, size, bold)`), else the
    /// default `FixedSys`, 8, not bold. `None` when they already existed.
    pub allocate_strings_with_font: Option<(String, i32, bool)>,
    /// `m_benchmarkDisplayString == NULL` before the call: it is created with `FixedSys`, 8, not bold.
    pub allocate_benchmark_string: bool,
}

/// C's `%g` for a double (6 significant digits, trailing zeros removed, exponent form when the exponent is below -4 or
/// at least 6).
pub fn format_g(v: f64) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    if !v.is_finite() {
        return if v.is_nan() { "nan".to_string() } else if v > 0.0 { "inf".to_string() } else { "-inf".to_string() };
    }
    let sci = format!("{:.5e}", v); // d.ddddde<exp>, rounded to 6 significant digits
    let (mantissa, exp) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let strip = |s: &str| -> String {
        if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s.to_string() }
    };
    if exp < -4 || exp >= 6 {
        format!("{}e{}{:02}", strip(mantissa), if exp < 0 { '-' } else { '+' }, exp.abs())
    } else {
        let decimals = (5 - exp).max(0) as usize;
        strip(&format!("{:.*}", decimals, v))
    }
}

/// `W3DDisplay::gatherDebugStats` (W3DDisplay.cpp:936): computes the debug text lines, at most every 2 seconds (or every
/// call with `m_constantDebugUpdate`). Counters accumulate on every call; the strings are rebuilt only when the update
/// delay has passed, after which the counters and timer reset. Release build (`#else`) text is used for the FPS line and
/// the selected-object line; the `EXTENDED_STATS` machinery is not compiled. `strings_allocated` /
/// `benchmark_allocated` are `m_displayStrings[0] != NULL` / `m_benchmarkDisplayString != NULL` and are set here.
pub fn gather_debug_stats(
    state: &mut GatherDebugStatsState,
    strings_allocated: &mut bool,
    benchmark_allocated: &mut bool,
    inp: &GatherDebugStatsInputs,
) -> GatherDebugStatsOutput {
    let mut out = GatherDebugStatsOutput::default();

    // allocate the display strings if needed
    if !*strings_allocated {
        let font = match &inp.native_debug_font {
            Some((name, size, bold)) if !name.is_empty() => (name.clone(), *size, *bold),
            _ => ("FixedSys".to_string(), 8, false),
        };
        // upstream: TheDisplayStringManager->newDisplayString() x DisplayStringCount, setFont - device-only, not translated
        out.allocate_strings_with_font = Some(font);
        *strings_allocated = true;
    }
    if !*benchmark_allocated {
        out.allocate_benchmark_string = true; // FixedSys, 8, FALSE
        *benchmark_allocated = true;
    }

    state.frames_rendered_since_last_update += 1;
    state.draw_calls_since_last_update += inp.draw_calls;
    state.sorted_polys_since_last_update += inp.sorting_polygons;

    let freq64 = inp.freq64;
    let time64 = inp.time64;

    state.time_since_last_update_in_secs = (time64 - state.last_update_time64) as f64 / freq64 as f64;

    // we update stats on a delay
    let update_rate_secs: f32 = 2.0;
    if state.time_since_last_update_in_secs >= update_rate_secs as f64 || inp.constant_debug_update {
        // upstream: Debug_Statistics::Record_Texture_Mode(RECORD_TEXTURE_SIMPLE) - device-only, not translated

        // frames per second
        let mut fps = state.frames_rendered_since_last_update as f32 as f64 / state.time_since_last_update_in_secs;
        let draws_per_frame = inp.draw_calls as f64;
        let sort_polys_per_frame = inp.sorting_polygons as f64;
        let skin_draws_per_frame = inp.skin_renders as f64;

        if fps < 0.1 {
            fps = 0.1;
        }
        let ms = 1000.0f32 as f64 / fps;

        // Release arm (#else): the _DEBUG/_INTERNAL arm with cumulative FPS is not compiled.
        let mut unibuffer = format!(
            "FPS: {:.2}, {:.2}ms draws: {:.2} skins: {:.2} sort {:.2}",
            fps, ms, draws_per_frame, skin_draws_per_frame, sort_polys_per_frame
        );
        if inp.use_fps_limit {
            unibuffer.push_str(&format!(", FPSLock {}", inp.frames_per_second_limit));
        }

        out.benchmark_text = Some(format!("FPS: {:.2}", fps));

        let poly_per_frame = inp.polygons;

        // (EXTENDED_STATS block: macro not defined, not translated)

        // check for debug D3D
        if inp.debug_d3d {
            unibuffer.push_str(", DEBUG D3D");
        }
        // #ifdef _DEBUG: ", DEBUG app" is not compiled in Release.

        out.texts.push((DebugString::Fps, unibuffer));

        // Actual GameLogic frame number
        out.texts.push((DebugString::Frame, format!("Frame: {}", inp.game_logic_frame)));

        // polygons this frame
        out.texts.push((
            DebugString::Polygons,
            format!("Polygons: per frame {}, per second {}", poly_per_frame, (poly_per_frame as f64 * fps) as i32),
        ));

        // vertices this frame
        out.texts.push((DebugString::Vertices, format!("Vertices: {}", inp.vertices)));

        // I'm adjusting the texture memory usage counter by subtracting out the terrain alpha texture (since it's
        // really == terrain texture).
        out.texts.push((DebugString::VideoRam, format!("Video RAM: {}", inp.record_texture_size - 1376256)));

        state.last_update_time64 = time64;
        state.time_since_last_update_in_secs = 0.0;
        state.frames_rendered_since_last_update = 0;
        state.draw_calls_since_last_update = 0;
        state.sorted_polys_since_last_update = 0;

        // terrain stats
        out.texts.push((
            DebugString::TerrainStats,
            format!(
                "3-Way Blends: {}/{}, Shoreline Blends: {}/{}",
                inp.extra_blend_tiles.0, inp.extra_blend_tiles.1, inp.shore_line_tiles.0, inp.shore_line_tiles.1
            ),
        ));

        // misc debug info
        out.texts.push((
            DebugString::DebugInfo,
            format!(
                "Camera zoom: {}, pitch: {}/{}, yaw: {}, pos: {}, {}, {}, FOV: {}\n       Height above ground: {} Terrain height: {}",
                format_g(inp.zoom as f64),
                format_g(inp.pitch as f64),
                format_g(inp.fx_pitch as f64),
                format_g(inp.angle as f64),
                format_g(inp.cam_pos.0 as f64),
                format_g(inp.cam_pos.1 as f64),
                format_g(inp.cam_pos.2 as f64),
                format_g(inp.fov as f64),
                format_g(inp.actual_height_above_ground as f64),
                format_g(inp.terrain_height as f64),
            ),
        ));

        // display the keyboard modifier and mouse states.
        let mut unibuffer = String::from("States: ");
        if inp.shift {
            unibuffer.push_str("Shift(");
            if inp.l_shift {
                unibuffer.push('L');
            }
            if inp.r_shift {
                unibuffer.push('R');
            }
            unibuffer.push_str(") ");
        }
        if inp.ctrl {
            unibuffer.push_str("Ctrl(");
            if inp.l_control {
                unibuffer.push('L');
            }
            if inp.r_control {
                unibuffer.push('R');
            }
            unibuffer.push_str(") ");
        }
        if inp.alt {
            unibuffer.push_str("Alt(");
            if inp.l_alt {
                unibuffer.push('L');
            }
            if inp.r_alt {
                unibuffer.push('R');
            }
            unibuffer.push_str(") ");
        }
        if inp.left_state {
            unibuffer.push_str("LMB ");
        }
        if inp.middle_state {
            unibuffer.push_str("MMB ");
        }
        if inp.right_state {
            unibuffer.push_str("RMB ");
        }

        // Release arm: Drawable *draw = TheGameClient->findDrawableByID(TheInGameUI->getMousedOverDrawableID());
        let mut draw: Option<&DebugDrawableInfo> = inp.moused_over_drawable.as_ref();
        let object = draw.and_then(|d| d.object_for_mouse_over.as_ref());
        if let Some((template_name, id)) = object {
            unibuffer.push_str(&format!("Moused over object: {} ({}) ", template_name, id));
        } else {
            unibuffer.push_str("Moused over object: TERRAIN ");
        }
        out.texts.push((DebugString::KeyMouseStates, unibuffer));

        // display the x and y mouse coordinates
        out.texts.push((
            DebugString::MousePosition,
            format!(
                "Mouse position: screen: ({}, {}), world: ({}, {}, {})",
                inp.mouse_pos.0,
                inp.mouse_pos.1,
                format_g(inp.mouse_world_pos.0 as f64),
                format_g(inp.mouse_world_pos.1 as f64),
                format_g(inp.mouse_world_pos.2 as f64),
            ),
        ));

        // display the number of particles in the world and being displayed on screen
        out.texts.push((
            DebugString::Particles,
            format!("Particles: {} in world, {} being displayed", inp.particle_count, inp.on_screen_particle_count),
        ));

        // display the number of objects in the world
        out.texts.push((
            DebugString::Objects,
            format!("Objects: {} in world, {} being displayed", inp.object_count, inp.rendered_object_count),
        ));

        // Network incoming bandwidth stats
        if let Some(net) = &inp.network {
            out.texts.push((
                DebugString::NetIncoming,
                format!("IN: {:.2} bytes/sec, {:.2} packets/sec", net.incoming_bytes_per_second, net.incoming_packets_per_second),
            ));
            // Network outgoing bandwidth stats
            out.texts.push((
                DebugString::NetOutgoing,
                format!("OUT: {:.2} bytes/sec, {:.2} packets/sec", net.outgoing_bytes_per_second, net.outgoing_packets_per_second),
            ));
            // Network performance stats
            out.texts.push((
                DebugString::NetStats,
                format!(
                    "Run Ahead: {}, Net FPS: {}, Packet arrival cushion: {}",
                    net.run_ahead, net.frame_rate, net.packet_arrival_cushion
                ),
            ));
            // Client frame rate averages for all players in the game. This only works right for the packet router.
            let mut unibuffer = String::new();
            for (name, avg_fps) in &net.players {
                unibuffer.push_str(&format!("{}: {} ", name, avg_fps));
            }
            out.texts.push((DebugString::NetFpsAverages, unibuffer));
        } else {
            // Network not present: all four strings are set to the empty string
            out.texts.push((DebugString::NetOutgoing, String::new()));
            out.texts.push((DebugString::NetIncoming, String::new()));
            out.texts.push((DebugString::NetStats, String::new()));
            out.texts.push((DebugString::NetFpsAverages, String::new()));
        }

        // selected object info stats
        let mut unibuffer = format!("Select Info: '{}' drawables selected", inp.select_count);

        // Sorry, guys. I need a special kluge here to get constantdebug results for angry mob.
        // if there is not exactly one drawable selected it will report on the moused-over drawable
        if inp.select_count == 1 {
            draw = inp.first_selected_drawable.as_ref();
        }

        if let Some(draw) = draw {
            let object_name = match &draw.object_name {
                Some(n) if !n.is_empty() => n.clone(),
                _ => "No-Name".to_string(),
            };
            unibuffer = format!(
                "Select Info: '{}'({}) at ({:.3},{:.3},{:.3})",
                draw.template_name, object_name, draw.position.0, draw.position.1, draw.position.2
            );

            if let Some((pitch, pitch_rate, roll, roll_rate)) = draw.loco_info {
                unibuffer.push_str(&format!(
                    "\nPhysics Info -- Turn: {}, Pitch(accel): {:.3}({:.3}), Roll(accel): {:.3}({:.3})",
                    draw.turn_type, pitch, pitch_rate, roll, roll_rate
                ));
            }

            // (gth) render cost of the drawable: #if defined(_DEBUG) || defined(_INTERNAL) only, not compiled.

            unibuffer.push_str("\nModelStates: ");
            let num_entries_per_line = 4;
            let mut line_count = 0;
            for name in &draw.model_condition_names {
                unibuffer.push_str(&format!("{} ", name));
                line_count += 1;
                if line_count == num_entries_per_line {
                    line_count = 0;
                    unibuffer.push('\n');
                }
            }
        } // end if
        out.texts.push((DebugString::SelectedInfo, unibuffer));
    }

    out
}
