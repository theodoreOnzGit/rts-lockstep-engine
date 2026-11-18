//! UNTESTED DRAFT (#202, #203, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/LoadScreen.cpp (PARTIAL, see below)
//!     drafted: the frame constants, `LoadScreen` (`~LoadScreen`, `update`), `SinglePlayerLoadScreen` (all),
//!       `ChallengeLoadScreen::reset/update/setProgressRange`, `ShellGameLoadScreen` (all),
//!       `MultiPlayerLoadScreen::reset/update/processProgress`, `GameSpyLoadScreen::reset/update/processProgress`,
//!       `MapTransferLoadScreen` (all)
//!     ~~NOT drafted yet~~ **Drafted (#203, second pass, lines 644-1138, 1286-1496, 1544-1865 of the upstream file):**
//!       `ChallengeLoadScreen`'s constructor, destructor, `updateTeletypeText`, `activatePieces`,
//!       `activatePiecesMinSpec`, `init`; `MultiPlayerLoadScreen`'s constructor, destructor and `init`;
//!       `GameSpyLoadScreen`'s constructor, destructor and `init`. They are at the end of this file; the members the
//!       shared [`LoadScreen`] lacks live in [`ChallengeLoadScreenWindows`], [`MultiPlayerExtra`] and
//!       [`GameSpyExtra`].
//!
//! The video (`TheVideoPlayer`, Bink) and the audio handle (Miles) are the host's; their calls are marked
//! `// upstream: ... — device-only, not translated`. The blocking loops (`Sleep`, `serviceWindowsOS`) are kept as
//! loops over the host's calls.

use crate::client::gadget;
use crate::client::window_manager::{GameWindowManager, GuiHost};
use crate::client::window_script::WinId;
use crate::drafts::ui::window_video_manager::{VideoHost, WindowVideoManager, WindowVideoPlayType};

/// `MAX_SLOTS` (GameInfo.h).
pub const MAX_SLOTS: usize = 8;
/// `MAX_OBJECTIVE_LINES`, `MAX_DISPLAYED_UNITS` (LoadScreen.h).
pub const MAX_OBJECTIVE_LINES: usize = 5;
pub const MAX_DISPLAYED_UNITS: usize = 3;

/// LoadScreen.cpp's frame enum.
pub const FRAME_TITLES_START: i32 = 20;
pub const FRAME_TELETYPE_START: i32 = 24;
pub const FRAME_FUDGE_ADD: i32 = 30;
pub const FRAME_PORTRAITS_START: i32 = 35;
pub const FRAME_OUTER_CIRCLE_LINE_SHOW: i32 = 50;
pub const FRAME_INNER_CIRCLE_LINE_SHOW: i32 = 52;
pub const FRAME_OUTER_CIRCLE_ALPHA_SHOW: i32 = 63;
pub const FRAME_INNER_CIRCLE_ALPHA_SHOW: i32 = 74;
pub const FRAME_OUTER_CIRCLE_LINE_HIDE: i32 = 75;
pub const FRAME_INNER_BACKDROP_ALPHA_SHOW: i32 = 80;
pub const FRAME_INNER_CIRCLE_LINE_HIDE: i32 = 81;
pub const FRAME_VS_ANIM_START: i32 = 98;
pub const FRAME_RIGHT_VOICE: i32 = 140;
/// `TELETYPE_UPDATE_FREQ`: how many frames between teletype updates.
pub const TELETYPE_UPDATE_FREQ: i32 = 2;

/// `GameMakeColor(...)` of an enabled bar is a host call; colours are plain ints here.
pub type Color = i32;

/// `LoadScreen::m_loadScreen` and the members of the derived classes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoadScreen {
    /// `m_loadScreen`.
    pub load_screen: Option<WinId>,
    pub progress_bar: Option<WinId>,
    // SinglePlayerLoadScreen
    pub percent: Option<WinId>,
    pub objective_win: Option<WinId>,
    pub objective_lines: [Option<WinId>; MAX_OBJECTIVE_LINES],
    pub unicode_objective_lines: [Vec<u16>; MAX_OBJECTIVE_LINES],
    pub unit_desc: [Option<WinId>; MAX_DISPLAYED_UNITS],
    pub location: Option<WinId>,
    pub current_objective_line: usize,
    pub current_objective_line_character: usize,
    pub current_objective_width_offset: i32,
    pub finished_objective_text: bool,
    /// The video stream and buffer handles (`m_videoStream`, `m_videoBuffer`): the host's.
    pub video_stream: Option<usize>,
    pub video_buffer: Option<usize>,
    /// `m_ambientLoopHandle` (`NULL` is `None`).
    pub ambient_loop_handle: Option<usize>,
    // the multiplayer, GameSpy and map transfer screens
    pub progress_bars: [Option<WinId>; MAX_SLOTS],
    pub player_names: [Option<WinId>; MAX_SLOTS],
    pub player_side: [Option<WinId>; MAX_SLOTS],
    pub progress_text: [Option<WinId>; MAX_SLOTS],
    pub player_lookup: [i32; MAX_SLOTS],
    pub old_progress: [i32; MAX_SLOTS],
    pub old_timeout: i32,
    pub file_name_text: Option<WinId>,
    pub timeout_text: Option<WinId>,
}

impl LoadScreen {
    /// `LoadScreen::LoadScreen()` and the constructors' defaults (`m_playerLookup` and `m_oldProgress` are -1 as the
    /// map transfer screen's constructor sets them; the multiplayer screens' `init` assigns their lookup).
    pub fn new() -> Self {
        LoadScreen { player_lookup: [-1; MAX_SLOTS], old_progress: [-1; MAX_SLOTS], ..Default::default() }
    }
}

/// The mission of `TheCampaignManager->getCurrentMission()` as `SinglePlayerLoadScreen` reads it.
#[derive(Clone, Debug, Default)]
pub struct MissionInfo {
    /// `m_missionObjectivesLabel[i]` (empty: `isNotEmpty()` false).
    pub mission_objectives_label: [Vec<u8>; MAX_OBJECTIVE_LINES],
    pub unit_names: [Vec<u8>; MAX_DISPLAYED_UNITS],
    pub location_name_label: Vec<u8>,
    pub movie_label: Vec<u8>,
    /// `m_voiceLength` (seconds).
    pub voice_length: i32,
    /// `getCurrentCampaign()->m_name`.
    pub campaign_name: Vec<u8>,
}

/// What the load screens reach outside the window manager.
pub trait LoadScreenHost: GuiHost {
    fn load_screen(&mut self) -> &mut LoadScreen;
    fn name_to_key(&mut self, name: &[u8]) -> i32;
    /// `TheWindowManager->winCreateFromScript(filename)`.
    fn win_create_from_script(&mut self, wm: &mut GameWindowManager, filename: &[u8]) -> Option<WinId>;
    /// `TheGameText->fetch(label)`.
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    /// `TheMouse->setCursorTooltip(UnicodeString::TheEmptyString)`.
    fn mouse_clear_cursor_tooltip(&mut self);
    /// `TheGameEngine->serviceWindowsOS()`, `getQuitting()`, `isActive()`.
    fn service_windows_os(&mut self);
    fn game_engine_quitting(&self) -> bool;
    fn game_engine_is_active(&self) -> bool;
    /// `TheWindowManager->update()` is `wm.update(h)`; `TheDisplay->update()`, `draw()`.
    fn display_update(&mut self);
    fn display_draw(&mut self);
    /// `setFPMode()` (FPUControl.h).
    fn set_fp_mode(&mut self);
    /// `Sleep(ms)`, `timeGetTime()`.
    fn sleep_ms(&mut self, ms: u32);
    fn time_get_time(&self) -> u32;
    /// `TheCampaignManager->getCurrentMission()`.
    fn current_mission(&self) -> MissionInfo;
    /// `TheAudio->friend_forcePlayAudioEventRTS(&mission->m_briefingVoice)`.
    fn audio_force_play_briefing_voice(&mut self);
    /// `m_ambientLoop.setEventName("LoadScreenAmbient")`, `TheAudio->addAudioEvent(&m_ambientLoop)` (a handle).
    fn audio_add_ambient_loop(&mut self) -> Option<usize>;
    /// `TheAudio->removeAudioEvent(handle)`.
    fn audio_remove_event(&mut self, handle: usize);
    // video: all device-only (Bink)
    /// `TheVideoPlayer->open(movie)`, `TheDisplay->createVideoBuffer()`, `allocate(width, height)`; the handles, or
    /// `None` when any step failed (the half-made stream and buffer are released, as upstream does).
    fn video_open_stream_and_buffer(&mut self, movie: &[u8]) -> Option<(usize, usize)>;
    fn video_frame_count(&self, stream: usize) -> i32;
    fn video_frame_index(&self, stream: usize) -> i32;
    fn video_is_frame_ready(&mut self, stream: usize) -> bool;
    /// `frameDecompress()`, `frameRender(buffer)`, `frameNext()`.
    fn video_decompress_render(&mut self, stream: usize, buffer: usize);
    fn video_frame_next(&mut self, stream: usize);
    /// `videoStream->close()`; `delete m_videoBuffer`.
    fn video_close(&mut self, stream: usize, buffer: usize);
    /// `m_loadScreen->winGetInstanceData()->setVideoBuffer(buffer)`.
    fn win_set_video_buffer(&mut self, wm: &mut GameWindowManager, w: WinId, buffer: Option<usize>);
    /// `TheGameLODManager && TheGameLODManager->didMemPass()`.
    fn game_lod_did_mem_pass(&self) -> bool;
    /// `win->winSetEnabledImage(index, TheMappedImageCollection->findImageByName(name))`.
    fn win_set_enabled_image_by_name(&mut self, wm: &mut GameWindowManager, w: WinId, index: usize, name: &[u8]);
    /// `TheWritableGlobalData->m_breakTheMovie = FALSE`.
    fn global_break_the_movie_clear(&mut self);
    /// `GadgetProgressBarSetEnabledBarColor(w, color)`.
    fn gadget_progress_bar_set_enabled_bar_color(&mut self, wm: &mut GameWindowManager, w: WinId, color: Color);
    /// `w->winSetEnabledTextColors(color, w->winGetEnabledTextBorderColor())`.
    fn win_set_enabled_text_color_keep_border(&mut self, wm: &mut GameWindowManager, w: WinId, color: Color);
    /// `TheMultiplayerSettings->getColor(slot->getApparentColor())->getColor()`.
    fn slot_house_color(&self, game_slot: usize) -> Color;
    /// `game->getSlot(i)`: `Some((name, isHuman))`; `None` for no slot.
    fn game_slot(&self, i: usize) -> Option<(Vec<u16>, bool)>;
    /// `TheGameInfo->getConstSlot(i)->isHuman() && ->hasMap()`.
    fn game_slot_human_has_map(&self, i: usize) -> bool;
    /// `TheNetwork` present; `updateLoadProgress(percent)`, `liteupdate()`.
    fn has_network(&self) -> bool;
    fn network_update_load_progress(&mut self, percent: i32);
    fn network_lite_update(&mut self);
    /// `TheGameLogic->processProgress(TheGameInfo->getLocalSlotNum(), percent)`.
    fn game_logic_process_progress_local(&mut self, percent: i32);
    /// `TheGameState->getMapLeafName(filename)`.
    fn game_state_map_leaf_name(&self, filename: &[u8]) -> Vec<u16>;
    /// `UnicodeString::format(fmt, ...)` for the two map transfer labels.
    fn format_with_text(&self, fmt: &[u16], text: &[u16]) -> Vec<u16>;
    fn format_minutes_seconds(&self, fmt: &[u16], minutes: i32, seconds: i32) -> Vec<u16>;
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn win_in<H: LoadScreenHost>(wm: &GameWindowManager, h: &mut H, parent: Option<WinId>, name: &str) -> Option<WinId> {
    let id = h.name_to_key(name.as_bytes());
    wm.win_get_window_from_id(parent, id)
}

/// `LoadScreen::~LoadScreen()`: `winDestroy(m_loadScreen)`.
pub fn load_screen_destroy<H: LoadScreenHost>(wm: &mut GameWindowManager, h: &mut H) {
    if let Some(w) = h.load_screen().load_screen.take() {
        wm.win_destroy(Some(w));
    }
}

/// `LoadScreen::update(percent)` (LoadScreen.cpp:164).
pub fn load_screen_update<H: LoadScreenHost>(wm: &mut GameWindowManager, h: &mut H, _percent: i32) {
    h.service_windows_os();
    if h.game_engine_quitting() {
        return; // don't bother with any of this if the player is exiting game.
    }

    wm.update(h);
    h.display_update();
    // redraw all views, update the GUI
    h.display_draw();

    h.set_fp_mode();
}

/// `LoadScreen::setLoadScreen(NULL)` as `reset` calls it (the window is destroyed by the owner's manager).
fn set_load_screen_none<H: LoadScreenHost>(h: &mut H) {
    h.load_screen().load_screen = None;
}

// SinglePlayerLoadScreen ///////////////////////////////////////////////////////////////////////

/// `SinglePlayerLoadScreen::~SinglePlayerLoadScreen()`.
pub fn single_player_destroy<H: LoadScreenHost>(h: &mut H) {
    {
        let l = h.load_screen();
        l.progress_bar = None;
        l.percent = None;
        l.objective_win = None;
        l.objective_lines = [None; MAX_OBJECTIVE_LINES];
    }
    if let (Some(stream), Some(buffer)) = (h.load_screen().video_stream.take(), h.load_screen().video_buffer.take()) {
        h.video_close(stream, buffer);
    }
    if let Some(handle) = h.load_screen().ambient_loop_handle.take() {
        h.audio_remove_event(handle);
    }
}

/// `SinglePlayerLoadScreen::moveWindows(frame)` (LoadScreen.cpp:220).
pub fn single_player_move_windows<H: LoadScreenHost>(wm: &mut GameWindowManager, h: &mut H, frame: i32) {
    const STATE_BEGIN: i32 = 250;
    const STATE_SHOW_LOCATION: i32 = 251;
    const STATE_BEGIN_BREIFING: i32 = 255;
    const STATE_SHOW_CAMEO_1: i32 = 434;
    const STATE_BEGIN_ANIMATING_TEXT: i32 = 356;
    const STATE_HIDE_CAMEO_1: i32 = 459;
    const STATE_SHOW_CAMEO_2: i32 = 464;
    const STATE_HIDE_CAMEO_2: i32 = 492;
    const STATE_SHOW_CAMEO_3: i32 = 497;
    const STATE_HIDE_CAMEO_3: i32 = 524;
    const STATE_END_ANIMATING_TEXT: i32 = 730;
    const STATE_END: i32 = 730;
    if frame < STATE_BEGIN || frame > STATE_END {
        return;
    }

    if frame == STATE_BEGIN_BREIFING {
        // add sound support here
        h.audio_force_play_briefing_voice();
    }

    if frame == STATE_BEGIN_ANIMATING_TEXT {
        if let Some(w) = h.load_screen().objective_win {
            wm.win_hide(w, false);
        }
        // animate the text and stuff
    }

    if frame > STATE_BEGIN_ANIMATING_TEXT && frame <= STATE_END_ANIMATING_TEXT && !h.load_screen().finished_objective_text {
        {
            let l = h.load_screen();
            if l.current_objective_line < MAX_OBJECTIVE_LINES && l.current_objective_line_character >= l.unicode_objective_lines[l.current_objective_line].len() {
                l.current_objective_line += 1;
                l.current_objective_line_character = 0;
            }
        }
        let l = h.load_screen().clone();
        if l.current_objective_line >= MAX_OBJECTIVE_LINES || l.unicode_objective_lines[l.current_objective_line].is_empty() {
            h.load_screen().finished_objective_text = true;
        } else {
            let w_char = l.unicode_objective_lines[l.current_objective_line][l.current_objective_line_character];
            let target = l.objective_lines[l.current_objective_line];
            let mut text = target.map(|t| gadget::static_text_get_text(wm, t)).unwrap_or_default();
            text.push(w_char);
            gadget::static_text_set_text(wm, h, target, &text);
        }
        h.load_screen().current_objective_line_character += 1;
    }
    let unit_desc = h.load_screen().unit_desc;
    let location = h.load_screen().location;
    match frame {
        STATE_SHOW_LOCATION => {
            if let Some(w) = location {
                wm.win_hide(w, false);
            }
        }
        STATE_SHOW_CAMEO_1 => hide_opt(wm, unit_desc[0], false),
        STATE_HIDE_CAMEO_1 => hide_opt(wm, unit_desc[0], true),
        STATE_SHOW_CAMEO_2 => hide_opt(wm, unit_desc[1], false),
        STATE_HIDE_CAMEO_2 => hide_opt(wm, unit_desc[1], true),
        STATE_SHOW_CAMEO_3 => hide_opt(wm, unit_desc[2], false),
        STATE_HIDE_CAMEO_3 => hide_opt(wm, unit_desc[2], true),
        _ => {}
    }
}

fn hide_opt(wm: &mut GameWindowManager, w: Option<WinId>, hide: bool) {
    if let Some(w) = w {
        wm.win_hide(w, hide);
    }
}

/// `SinglePlayerLoadScreen::init(game)` (LoadScreen.cpp:392). The commented-out cameo windows are not translated.
pub fn single_player_init<H: LoadScreenHost>(wm: &mut GameWindowManager, h: &mut H) {
    // No music in SinglePlayerLoadScreen

    // create the layout of the load screen
    let ls = h.win_create_from_script(wm, b"Menus/SinglePlayerLoadScreen.wnd");
    h.load_screen().load_screen = ls;
    let Some(ls) = ls else { return }; // (DEBUG_ASSERTCRASH: upstream dereferences NULL)
    wm.win_hide(ls, false);
    wm.win_bring_to_top(ls);

    // Store the pointer to the progress bar on the loadscreen
    let progress_bar = win_in(wm, h, Some(ls), "SinglePlayerLoadScreen.wnd:ProgressLoad");
    h.load_screen().progress_bar = progress_bar;
    if let Some(p) = progress_bar {
        gadget::progress_bar_set_progress(wm, h, p, 0);
    }

    let percent = win_in(wm, h, Some(ls), "SinglePlayerLoadScreen.wnd:Percent");
    h.load_screen().percent = percent;
    gadget::static_text_set_text(wm, h, percent, &utf16("0%"));
    hide_opt(wm, percent, true);

    let objective_win = win_in(wm, h, Some(ls), "SinglePlayerLoadScreen.wnd:ObjectivesWin");
    h.load_screen().objective_win = objective_win;
    hide_opt(wm, objective_win, true);

    let mission = h.current_mission();
    for i in 0..MAX_OBJECTIVE_LINES {
        let line = win_in(wm, h, Some(ls), &format!("SinglePlayerLoadScreen.wnd:StaticTextLine{i}"));
        h.load_screen().objective_lines[i] = line;
        gadget::static_text_set_text(wm, h, line, &[]);

        // translate the objective lines
        if !mission.mission_objectives_label[i].is_empty() {
            let t = h.fetch_text(&mission.mission_objectives_label[i]);
            h.load_screen().unicode_objective_lines[i] = t;
        }
    }

    for i in 0..MAX_DISPLAYED_UNITS {
        let desc = win_in(wm, h, Some(ls), &format!("SinglePlayerLoadScreen.wnd:StaticTextCameoText{i}"));
        h.load_screen().unit_desc[i] = desc;
        let t = h.fetch_text(&mission.unit_names[i]);
        gadget::static_text_set_text(wm, h, desc, &t);
        hide_opt(wm, desc, true);
    }
    let location = win_in(wm, h, Some(ls), "SinglePlayerLoadScreen.wnd:StaticTextCameoText3");
    h.load_screen().location = location;
    hide_opt(wm, location, true);
    let t = h.fetch_text(&mission.location_name_label);
    gadget::static_text_set_text(wm, h, location, &t);

    {
        let l = h.load_screen();
        l.current_objective_line = 0;
        l.current_objective_width_offset = 0;
        l.current_objective_line_character = 0;
        l.finished_objective_text = false;
    }

    // upstream: m_ambientLoop.setEventName("LoadScreenAmbient") — set in audio_add_ambient_loop
    // create the new stream and the new buffer
    let Some((stream, buffer)) = h.video_open_stream_and_buffer(&mission.movie_label) else {
        hide_opt(wm, percent, true);
        return;
    };
    h.load_screen().video_stream = Some(stream);
    h.load_screen().video_buffer = Some(buffer);

    // format the progress bar: USA to blue, GLA to green, China to red and set the background image
    let campaign_name = mission.campaign_name.to_ascii_lowercase();
    let background_win = win_in(wm, h, Some(ls), "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen");
    if let (Some(bg), Some(pb)) = (background_win, progress_bar) {
        match campaign_name.as_slice() {
            b"usa" => {
                h.win_set_enabled_image_by_name(wm, bg, 0, b"MissionLoad_USA");
                h.win_set_enabled_image_by_name(wm, pb, 6, b"LoadingBar_ProgressCenter2");
            }
            b"gla" => {
                h.win_set_enabled_image_by_name(wm, bg, 0, b"MissionLoad_GLA");
                h.win_set_enabled_image_by_name(wm, pb, 6, b"LoadingBar_ProgressCenter3");
            }
            b"china" => {
                h.win_set_enabled_image_by_name(wm, bg, 0, b"MissionLoad_China");
                h.win_set_enabled_image_by_name(wm, pb, 6, b"LoadingBar_ProgressCenter1");
            }
            _ => {} // else leave the default background screen
        }
    }

    if h.game_lod_did_mem_pass() {
        let progress_update_count = h.video_frame_count(stream) / FRAME_FUDGE_ADD;
        let mut shifted_percent = -FRAME_FUDGE_ADD + 1;
        while h.video_frame_index(stream) < h.video_frame_count(stream) - 1 {
            h.service_windows_os();

            if !h.video_is_frame_ready(stream) {
                h.sleep_ms(1);
                continue;
            }

            if !h.game_engine_is_active() {
                // we are alt-tabbed out; changing for MissionDisk, just skip to end.
                break;
            }

            h.video_decompress_render(stream, buffer);

            // (PULLED FROM THE MISSION DISK upstream: moveWindows(m_videoStream->frameIndex()))

            h.video_frame_next(stream);

            h.win_set_video_buffer(wm, ls, Some(buffer));
            // (`% progressUpdateCount`: a zero count is a division by zero upstream; modelled as no update)
            if progress_update_count != 0 && h.video_frame_index(stream) % progress_update_count == 0 {
                shifted_percent += 1;
                if shifted_percent > 0 {
                    shifted_percent = 0;
                }
                let percent_value = ((shifted_percent + FRAME_FUDGE_ADD) as f64 / 1.3) as i32;
                let per = utf16(&format!("{percent_value}%"));
                h.mouse_clear_cursor_tooltip();
                if let Some(p) = progress_bar {
                    gadget::progress_bar_set_progress(wm, h, p, percent_value);
                }
                gadget::static_text_set_text(wm, h, percent, &per);
            }
            wm.update(h);

            // redraw all views, update the GUI
            h.display_draw();
        }

        // let the background image show through
        h.video_close(stream, buffer);
        h.load_screen().video_stream = None;
        h.load_screen().video_buffer = None;
        h.win_set_video_buffer(wm, ls, None);
        h.display_draw();
    } else {
        // if we're min spec'ed don't play a movie
        let delay = mission.voice_length * 1000;
        let begin = h.time_get_time() as i32;
        let mut curr_time = begin;
        while begin + delay > curr_time {
            let fudge_factor = (30.0 * ((curr_time - begin) as f32 / delay as f32)) as i32;
            if let Some(p) = progress_bar {
                gadget::progress_bar_set_progress(wm, h, p, fudge_factor);
            }

            wm.update(h);
            h.display_draw();
            h.sleep_ms(100);
            curr_time = h.time_get_time() as i32;
        }

        wm.update(h);
        h.display_draw();
    }
    h.set_fp_mode();
    hide_opt(wm, percent, true);
    let handle = h.audio_add_ambient_loop();
    h.load_screen().ambient_loop_handle = handle;
}

/// `SinglePlayerLoadScreen::reset()`.
pub fn single_player_reset<H: LoadScreenHost>(h: &mut H) {
    set_load_screen_none(h);
    h.load_screen().progress_bar = None;
}

/// `SinglePlayerLoadScreen::update(percent)`: `(percent + FRAME_FUDGE_ADD) / 1.3` (a double, truncated to Int).
pub fn single_player_update<H: LoadScreenHost>(wm: &mut GameWindowManager, h: &mut H, percent: i32) {
    let percent = ((percent + FRAME_FUDGE_ADD) as f64 / 1.3) as i32;
    let per = utf16(&format!("{percent}%"));
    h.mouse_clear_cursor_tooltip();
    let (bar, text) = (h.load_screen().progress_bar, h.load_screen().percent);
    if let Some(p) = bar {
        gadget::progress_bar_set_progress(wm, h, p, percent);
    }
    gadget::static_text_set_text(wm, h, text, &per);

    // Do this last!
    load_screen_update(wm, h, percent);
}

/// `SinglePlayerLoadScreen::setProgressRange(min, max)`: empty upstream.
pub fn single_player_set_progress_range(_min: i32, _max: i32) {}

// ChallengeLoadScreen //////////////////////////////////////////////////////////////////////////

/// `ChallengeLoadScreen::reset()`.
pub fn challenge_reset<H: LoadScreenHost>(h: &mut H) {
    set_load_screen_none(h);
    h.load_screen().progress_bar = None;
}

/// `ChallengeLoadScreen::update(percent)` (the unused `per` string is formatted upstream and dropped).
pub fn challenge_update<H: LoadScreenHost>(wm: &mut GameWindowManager, h: &mut H, percent: i32) {
    let percent = ((percent + FRAME_FUDGE_ADD) as f64 / 1.3) as i32;
    h.mouse_clear_cursor_tooltip();
    if let Some(p) = h.load_screen().progress_bar {
        gadget::progress_bar_set_progress(wm, h, p, percent);
    }

    // Do this last!
    load_screen_update(wm, h, percent);
}

/// `ChallengeLoadScreen::setProgressRange(min, max)`: empty upstream.
pub fn challenge_set_progress_range(_min: i32, _max: i32) {}

// ShellGameLoadScreen //////////////////////////////////////////////////////////////////////////

/// `ShellGameLoadScreen::init(game)` (LoadScreen.cpp:1175); `first_load` is the function's `static BOOL firstLoad`.
pub fn shell_game_init<H: LoadScreenHost>(wm: &mut GameWindowManager, h: &mut H, first_load: &mut bool) {
    // create the layout of the load screen
    let ls = h.win_create_from_script(wm, b"Menus/ShellGameLoadScreen.wnd");
    h.load_screen().load_screen = ls;
    let Some(ls) = ls else { return }; // (DEBUG_ASSERTCRASH: upstream dereferences NULL)
    wm.win_hide(ls, false);
    wm.win_bring_to_top(ls);

    // Store the pointer to the progress bar on the loadscreen
    let progress_bar = win_in(wm, h, Some(ls), "ShellGameLoadScreen.wnd:ProgressLoad");
    h.load_screen().progress_bar = progress_bar;
    if let Some(p) = progress_bar {
        gadget::progress_bar_set_progress(wm, h, p, 0);
    }
    hide_opt(wm, progress_bar, true);

    if *first_load && h.game_lod_did_mem_pass() {
        h.win_set_enabled_image_by_name(wm, ls, 0, b"TitleScreen");
        h.global_break_the_movie_clear();

        // (the Sizzle movie block is commented out upstream)
        let win = win_in(wm, h, Some(ls), "ShellGameLoadScreen.wnd:StaticTextLegal");
        hide_opt(wm, win, false);
        *first_load = false;

        let show_time = h.time_get_time();
        while show_time.wrapping_add(3000) > h.time_get_time() {
            load_screen_update(wm, h, 0);
            h.sleep_ms(100);
        }
    }
    hide_opt(wm, progress_bar, false);
}

/// `ShellGameLoadScreen::reset()`.
pub fn shell_game_reset<H: LoadScreenHost>(h: &mut H) {
    set_load_screen_none(h);
    h.load_screen().progress_bar = None;
}

/// `ShellGameLoadScreen::update(percent)`.
pub fn shell_game_update<H: LoadScreenHost>(wm: &mut GameWindowManager, h: &mut H, percent: i32) {
    h.mouse_clear_cursor_tooltip();
    if let Some(p) = h.load_screen().progress_bar {
        gadget::progress_bar_set_progress(wm, h, p, percent);
    }

    // Do this last!
    load_screen_update(wm, h, percent);
}

// MultiPlayerLoadScreen and GameSpyLoadScreen (reset, update, processProgress) /////////////////

/// `MultiPlayerLoadScreen::reset()` and `GameSpyLoadScreen::reset()`.
pub fn multi_player_reset<H: LoadScreenHost>(h: &mut H) {
    set_load_screen_none(h);
    let l = h.load_screen();
    for i in 0..MAX_SLOTS {
        l.progress_bars[i] = None;
        l.player_names[i] = None;
        l.player_side[i] = None;
    }
}

/// `MultiPlayerLoadScreen::update(percent)`: with `TheNetwork`, its load progress and `liteupdate()`; without, the
/// local slot's progress through the logic.
pub fn multi_player_update<H: LoadScreenHost>(wm: &mut GameWindowManager, h: &mut H, percent: i32) {
    if h.has_network() {
        if percent <= 100 {
            h.network_update_load_progress(percent);
        }
        h.network_lite_update();
    } else if percent <= 100 {
        h.game_logic_process_progress_local(percent);
    }

    h.mouse_clear_cursor_tooltip();

    // Do this last!
    load_screen_update(wm, h, percent);
}

/// `GameSpyLoadScreen::update(percent)`: `TheNetwork` is assumed present (upstream dereferences it).
pub fn game_spy_update<H: LoadScreenHost>(wm: &mut GameWindowManager, h: &mut H, percent: i32) {
    if percent <= 100 {
        h.network_update_load_progress(percent);
    }
    h.network_lite_update();

    h.mouse_clear_cursor_tooltip();

    // Do this last!
    load_screen_update(wm, h, percent);
}

/// `MultiPlayerLoadScreen::processProgress(playerId, percentage)` and `GameSpyLoadScreen::processProgress`: an
/// out-of-range id or percentage is a `DEBUG_ASSERTCRASH` (compiled out) and the code goes on to index with
/// `m_playerLookup[playerId]`; here an index outside the arrays is skipped instead of reading out of bounds.
pub fn multi_player_process_progress<H: LoadScreenHost>(wm: &mut GameWindowManager, h: &mut H, player_id: i32, percentage: i32) {
    if player_id < 0 || player_id as usize >= MAX_SLOTS {
        return;
    }
    let lookup = h.load_screen().player_lookup[player_id as usize];
    if lookup < 0 || lookup as usize >= MAX_SLOTS {
        return;
    }
    if let Some(bar) = h.load_screen().progress_bars[lookup as usize] {
        gadget::progress_bar_set_progress(wm, h, bar, percentage);
    }
}

// MapTransferLoadScreen ////////////////////////////////////////////////////////////////////////

/// `MapTransferLoadScreen::init(game)` (LoadScreen.cpp:1937).
pub fn map_transfer_init<H: LoadScreenHost>(wm: &mut GameWindowManager, h: &mut H) {
    // create the layout of the load screen
    let ls = h.win_create_from_script(wm, b"Menus/MapTransferScreen.wnd");
    h.load_screen().load_screen = ls;
    let Some(ls) = ls else { return };

    wm.win_hide(ls, false);
    wm.win_bring_to_top(ls);

    // Load the Filename Text
    let file_name_text = win_in(wm, h, Some(ls), "MapTransferScreen.wnd:StaticTextCurrentFile");
    // Load the Timeout Text
    let timeout_text = win_in(wm, h, Some(ls), "MapTransferScreen.wnd:StaticTextTimeout");
    h.load_screen().file_name_text = file_name_text;
    h.load_screen().timeout_text = timeout_text;

    let mut net_slot = 0usize;
    // Loop through and make the loadscreen look all good.
    for i in 0..MAX_SLOTS {
        // Load the Progress Bar, the Player's name and the Progress Text
        let bar = win_in(wm, h, Some(ls), &format!("MapTransferScreen.wnd:ProgressLoad{i}"));
        let name = win_in(wm, h, Some(ls), &format!("MapTransferScreen.wnd:StaticTextPlayer{i}"));
        let text = win_in(wm, h, Some(ls), &format!("MapTransferScreen.wnd:StaticTextProgress{i}"));
        {
            let l = h.load_screen();
            l.progress_bars[i] = bar;
            l.player_names[i] = name;
            l.progress_text[i] = text;
        }
        // set the progressbar to zero
        if let Some(b) = bar {
            gadget::progress_bar_set_progress(wm, h, b, 0);
        }

        // get the slot man!
        let Some((slot_name, is_human)) = h.game_slot(i) else { continue };
        if !is_human {
            continue;
        }
        let house_color = h.slot_house_color(i);
        let l = h.load_screen().clone();
        if let Some(b) = l.progress_bars[net_slot] {
            h.gadget_progress_bar_set_enabled_bar_color(wm, b, house_color);
        }

        gadget::static_text_set_text(wm, h, l.player_names[net_slot], &slot_name);
        if let Some(w) = l.player_names[net_slot] {
            h.win_set_enabled_text_color_keep_border(wm, w, house_color);
        }

        gadget::static_text_set_text(wm, h, l.progress_text[net_slot], &[]);
        if let Some(w) = l.progress_text[net_slot] {
            h.win_set_enabled_text_color_keep_border(wm, w, house_color);
        }

        if (i == 0 || (h.game_slot_human_has_map(i))) && l.progress_bars[net_slot].is_some() {
            hide_opt(wm, l.progress_bars[net_slot], true);
        }

        h.load_screen().player_lookup[i] = net_slot as i32; // save our mapping so we can update progress correctly

        net_slot += 1;
    }

    for i in net_slot..MAX_SLOTS {
        let l = h.load_screen().clone();
        hide_opt(wm, l.progress_bars[i], true);
        hide_opt(wm, l.player_names[i], true);
        hide_opt(wm, l.progress_text[i], true);
    }
}

/// `MapTransferLoadScreen::reset()`.
pub fn map_transfer_reset<H: LoadScreenHost>(h: &mut H) {
    set_load_screen_none(h);
    let l = h.load_screen();
    for i in 0..MAX_SLOTS {
        l.progress_bars[i] = None;
        l.player_names[i] = None;
        l.progress_text[i] = None;
        l.player_lookup[i] = -1;
        l.old_progress[i] = -1;
    }
    l.file_name_text = None;
    l.timeout_text = None;
}

/// `MapTransferLoadScreen::update(percent)`.
pub fn map_transfer_update<H: LoadScreenHost>(wm: &mut GameWindowManager, h: &mut H, percent: i32) {
    if h.has_network() {
        h.network_lite_update();
    }

    h.mouse_clear_cursor_tooltip();

    // Do this last!
    load_screen_update(wm, h, percent);
}

/// `MapTransferLoadScreen::processProgress(playerId, percentage, stateStr)`.
pub fn map_transfer_process_progress<H: LoadScreenHost>(wm: &mut GameWindowManager, h: &mut H, player_id: i32, percentage: i32, state_str: &[u8]) {
    // (the out-of-range case is a compiled-out assert upstream, which then indexes the arrays regardless)
    if player_id < 0 || player_id as usize >= MAX_SLOTS {
        return;
    }
    let pid = player_id as usize;
    if h.load_screen().old_progress[pid] == percentage {
        return;
    }
    h.load_screen().old_progress[pid] = percentage;

    let translated_slot = h.load_screen().player_lookup[pid];
    if translated_slot < 0 || translated_slot as usize >= MAX_SLOTS {
        return;
    }
    let l = h.load_screen().clone();
    if let Some(b) = l.progress_bars[translated_slot as usize] {
        gadget::progress_bar_set_progress(wm, h, b, percentage);
    }
    if l.progress_text[translated_slot as usize].is_some() {
        let t = h.fetch_text(state_str);
        gadget::static_text_set_text(wm, h, l.progress_text[translated_slot as usize], &t);
    }
}

/// `MapTransferLoadScreen::processTimeout(secondsLeft)`.
pub fn map_transfer_process_timeout<H: LoadScreenHost>(wm: &mut GameWindowManager, h: &mut H, seconds_left: i32) {
    if h.load_screen().old_timeout == seconds_left {
        return;
    }
    h.load_screen().old_timeout = seconds_left;

    if let Some(w) = h.load_screen().timeout_text {
        let fmt = h.fetch_text(b"MapTransfer:Timeout");
        let txt = h.format_minutes_seconds(&fmt, seconds_left / 60, seconds_left % 60);
        gadget::static_text_set_text(wm, h, Some(w), &txt);
    }
}

/// `MapTransferLoadScreen::setCurrentFilename(filename)`: the map's leaf name formatted into `MapTransfer:CurrentFile`.
pub fn map_transfer_set_current_filename<H: LoadScreenHost>(wm: &mut GameWindowManager, h: &mut H, filename: &[u8]) {
    if let Some(w) = h.load_screen().file_name_text {
        let leaf = h.game_state_map_leaf_name(filename);
        let fmt = h.fetch_text(b"MapTransfer:CurrentFile");
        let txt = h.format_with_text(&fmt, &leaf);
        gadget::static_text_set_text(wm, h, Some(w), &txt);
    }
}

// ChallengeLoadScreen: constructor, destructor, init ///////////////////////////////////////////

/// The members only `ChallengeLoadScreen` has (LoadScreen.h). The commented-out reticle line and `bioAge` windows
/// are not members of this port, as upstream comments them out of every use.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChallengeLoadScreenWindows {
    pub bio_name_left: Option<WinId>,
    pub bio_birthplace_left: Option<WinId>,
    pub bio_strategy_left: Option<WinId>,
    pub bio_big_name_entry_left: Option<WinId>,
    pub bio_name_entry_left: Option<WinId>,
    pub bio_birthplace_entry_left: Option<WinId>,
    pub bio_strategy_entry_left: Option<WinId>,
    pub bio_big_name_entry_right: Option<WinId>,
    pub bio_name_right: Option<WinId>,
    pub bio_birthplace_right: Option<WinId>,
    pub bio_strategy_right: Option<WinId>,
    pub bio_name_entry_right: Option<WinId>,
    pub bio_birthplace_entry_right: Option<WinId>,
    pub bio_strategy_entry_right: Option<WinId>,
    pub portrait_left: Option<WinId>,
    pub portrait_right: Option<WinId>,
    pub portrait_movie_left: Option<WinId>,
    pub portrait_movie_right: Option<WinId>,
    pub overlay_reticle_circle_alpha_outer: Option<WinId>,
    pub overlay_reticle_circle_alpha_inner: Option<WinId>,
    pub overlay_vs_backdrop: Option<WinId>,
    pub overlay_vs: Option<WinId>,
    /// `m_wndVideoManager` (`NULL` is `None`).
    pub wnd_video_manager: Option<WindowVideoManager>,
    /// `activatePieces`'s ten function-local statics (`textPosBigNameRight` ... `textPosStrategyLeft`).
    pub text_pos: TeletypePositions,
}

/// `activatePieces`'s statics. Order of the fields follows upstream's declaration order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TeletypePositions {
    pub big_name_right: i32,
    pub name_right: i32,
    pub age_right: i32,
    pub birthplace_right: i32,
    pub strategy_right: i32,
    pub big_name_left: i32,
    pub name_left: i32,
    pub age_left: i32,
    pub birthplace_left: i32,
    pub strategy_left: i32,
}

/// The `GeneralPersona` fields `ChallengeLoadScreen` reads (GeneralPersona in ChallengeGenerals.h).
#[derive(Clone, Debug, Default)]
pub struct GeneralPersonaInfo {
    /// `getNameSound()`, `getRandomTauntSound()` (the latter picks at random: the host's, evaluated when asked).
    pub name_sound: Vec<u8>,
    pub portrait_movie_left_name: Vec<u8>,
    pub portrait_movie_right_name: Vec<u8>,
    /// `getBioName()`, `getBioRank()`, `getBioStrategy()`: text labels.
    pub bio_name: Vec<u8>,
    pub bio_rank: Vec<u8>,
    pub bio_strategy: Vec<u8>,
    /// `getBioPortraitLarge()`: an image name the host looks up.
    pub bio_portrait_large: Vec<u8>,
}

/// What `ChallengeLoadScreen` reaches that the base host lacks.
pub trait ChallengeLoadScreenHost: LoadScreenHost + VideoHost {
    fn challenge_windows(&mut self) -> &mut ChallengeLoadScreenWindows;
    /// `TheChallengeGenerals->getPlayerGeneralByCampaignName(campaign->m_name)`.
    fn player_general_by_campaign_name(&self, campaign_name: &[u8]) -> GeneralPersonaInfo;
    /// `TheChallengeGenerals->getGeneralByGeneralName(mission->m_generalName)`.
    fn general_by_general_name(&self, general_name: &[u8]) -> GeneralPersonaInfo;
    /// `mission->m_generalName`.
    fn current_mission_general_name(&self) -> Vec<u8>;
    /// `generalOpponent->getRandomTauntSound()`.
    fn opponent_random_taunt_sound(&mut self, opponent: &GeneralPersonaInfo) -> Vec<u8>;
    /// `TheAudio->addAudioEvent(&AudioEventRTS(name))` (Miles audio: device-only, not translated).
    fn audio_add_event_named(&mut self, name: &[u8]);
    /// `TheAudio->update()`.
    fn audio_update(&mut self);
    /// `win->winSetEnabledImage(0, image)` with the image looked up by name; an empty name is `NULL`.
    fn win_set_enabled_image_named(&mut self, wm: &mut GameWindowManager, w: WinId, index: usize, name: &[u8]);
    /// `m_videoStream->frameGoto(m_videoStream->frameCount())`.
    fn video_frame_goto_end(&mut self, stream: usize);
}

/// `ChallengeLoadScreen::ChallengeLoadScreen()`: every member `NULL`.
pub fn challenge_new<H: ChallengeLoadScreenHost>(h: &mut H) {
    h.load_screen().progress_bar = None;
    h.load_screen().video_stream = None;
    h.load_screen().video_buffer = None;
    *h.challenge_windows() = ChallengeLoadScreenWindows::default();
}

/// `ChallengeLoadScreen::~ChallengeLoadScreen()`. The video manager is dropped (`delete m_wndVideoManager`).
pub fn challenge_destroy<H: ChallengeLoadScreenHost>(h: &mut H) {
    h.load_screen().progress_bar = None;
    if let (Some(stream), Some(buffer)) = (h.load_screen().video_stream.take(), h.load_screen().video_buffer.take()) {
        h.video_close(stream, buffer);
    }
    *h.challenge_windows() = ChallengeLoadScreenWindows::default();
    if let Some(handle) = h.load_screen().ambient_loop_handle.take() {
        h.audio_remove_event(handle);
    }
}

/// `updateTeletypeText(num_chars, window, full_text, current_text_pos)` (LoadScreen.cpp:732): appends up to
/// `num_chars` characters of `full_text` from `current_text_pos` to the window's text; returns the new position.
pub fn update_teletype_text<H: LoadScreenHost>(wm: &mut GameWindowManager, h: &mut H, num_chars: i32, window: Option<WinId>, full_text: &[u16], mut current_text_pos: i32) -> i32 {
    let mut current_text = match window {
        Some(w) => gadget::static_text_get_text(wm, w),
        None => Vec::new(),
    };
    for _ in 0..num_chars {
        if (current_text_pos as usize) < full_text.len() {
            let w_char = full_text[current_text_pos as usize];
            current_text.push(w_char);
            current_text_pos += 1;
        }
    }
    gadget::static_text_set_text(wm, h, window, &current_text);
    current_text_pos
}

/// `ChallengeLoadScreen::activatePieces(frame, generalPlayer, generalOpponent)` (LoadScreen.cpp:748).
pub fn challenge_activate_pieces<H: ChallengeLoadScreenHost>(wm: &mut GameWindowManager, h: &mut H, frame: i32, player: &GeneralPersonaInfo, opponent: &GeneralPersonaInfo) {
    let cw = h.challenge_windows().clone();
    match frame {
        FRAME_TITLES_START => {
            hide_opt(wm, cw.bio_name_left, false);
            hide_opt(wm, cw.bio_birthplace_left, false);
            hide_opt(wm, cw.bio_strategy_left, false);
            hide_opt(wm, cw.bio_name_right, false);
            hide_opt(wm, cw.bio_birthplace_right, false);
            hide_opt(wm, cw.bio_strategy_right, false);
        }
        FRAME_TELETYPE_START => {
            // reinit the statics for each new load screen
            h.challenge_windows().text_pos = TeletypePositions::default();

            for w in [cw.bio_big_name_entry_left, cw.bio_name_entry_left, cw.bio_birthplace_entry_left, cw.bio_strategy_entry_left] {
                hide_opt(wm, w, false);
            }
            for w in [cw.bio_big_name_entry_left, cw.bio_name_entry_left, cw.bio_birthplace_entry_left, cw.bio_strategy_entry_left] {
                gadget::static_text_set_text(wm, h, w, &[]);
            }

            for w in [cw.bio_big_name_entry_right, cw.bio_name_entry_right, cw.bio_birthplace_entry_right, cw.bio_strategy_entry_right] {
                hide_opt(wm, w, false);
            }
            for w in [cw.bio_big_name_entry_right, cw.bio_name_entry_right, cw.bio_birthplace_entry_right, cw.bio_strategy_entry_right] {
                gadget::static_text_set_text(wm, h, w, &[]);
            }
        }
        FRAME_PORTRAITS_START => {
            let mut mgr = h.challenge_windows().wnd_video_manager.take().unwrap_or_default();
            if let Some(w) = cw.portrait_movie_left {
                mgr.play_movie(wm, h, w, &player.portrait_movie_left_name, WindowVideoPlayType::ShowLastFrame);
            }
            if let Some(w) = cw.portrait_movie_right {
                mgr.play_movie(wm, h, w, &opponent.portrait_movie_right_name, WindowVideoPlayType::ShowLastFrame);
            }
            h.challenge_windows().wnd_video_manager = Some(mgr);
            hide_opt(wm, cw.portrait_movie_left, false);
            hide_opt(wm, cw.portrait_movie_right, false);

            // eventLeftGeneral( generalPlayer->getNameSound() )
            h.audio_add_event_named(&player.name_sound);
        }
        FRAME_OUTER_CIRCLE_LINE_SHOW => {} // m_overlayReticleCircleLineOuter->winHide(FALSE) is commented out upstream
        FRAME_INNER_CIRCLE_LINE_SHOW => {} // m_overlayReticleCircleLineInner->winHide(FALSE) is commented out upstream
        FRAME_OUTER_CIRCLE_ALPHA_SHOW => hide_opt(wm, cw.overlay_reticle_circle_alpha_outer, false),
        FRAME_INNER_CIRCLE_ALPHA_SHOW => hide_opt(wm, cw.overlay_reticle_circle_alpha_inner, false),
        FRAME_OUTER_CIRCLE_LINE_HIDE => {} // commented out upstream
        FRAME_INNER_BACKDROP_ALPHA_SHOW => hide_opt(wm, cw.overlay_vs_backdrop, false),
        FRAME_INNER_CIRCLE_LINE_HIDE => {} // commented out upstream
        FRAME_VS_ANIM_START => {
            // it's time to start the overlay movie
            hide_opt(wm, cw.overlay_vs_backdrop, false);
            hide_opt(wm, cw.overlay_vs, false);
            let mut mgr = h.challenge_windows().wnd_video_manager.take().unwrap_or_default();
            if let Some(w) = cw.overlay_vs {
                mgr.play_movie(wm, h, w, b"VSSmall", WindowVideoPlayType::ShowLastFrame);
            }
            h.challenge_windows().wnd_video_manager = Some(mgr);

            // "Verses": eventVS("Taunts_GCAnnouncer12")
            h.audio_add_event_named(b"Taunts_GCAnnouncer12");
        }
        FRAME_RIGHT_VOICE => h.audio_add_event_named(&opponent.name_sound),
        _ => {}
    }

    // update the teletype readout
    if frame > FRAME_TELETYPE_START && (frame % TELETYPE_UPDATE_FREQ) == 0 {
        let mut tp = h.challenge_windows().text_pos;
        let p_name = h.fetch_text(&player.bio_name);
        let p_rank = h.fetch_text(&player.bio_rank);
        let p_strategy = h.fetch_text(&player.bio_strategy);
        let o_name = h.fetch_text(&opponent.bio_name);
        let o_rank = h.fetch_text(&opponent.bio_rank);
        let o_strategy = h.fetch_text(&opponent.bio_strategy);

        tp.name_left = update_teletype_text(wm, h, 1, cw.bio_name_entry_left, &p_name, tp.name_left);
        tp.big_name_left = update_teletype_text(wm, h, 1, cw.bio_big_name_entry_left, &p_name, tp.big_name_left);
        tp.birthplace_left = update_teletype_text(wm, h, 1, cw.bio_birthplace_entry_left, &p_rank, tp.birthplace_left);
        tp.strategy_left = update_teletype_text(wm, h, 1, cw.bio_strategy_entry_left, &p_strategy, tp.strategy_left);

        tp.name_right = update_teletype_text(wm, h, 1, cw.bio_name_entry_right, &o_name, tp.name_right);
        tp.big_name_right = update_teletype_text(wm, h, 1, cw.bio_big_name_entry_right, &o_name, tp.big_name_right);
        tp.birthplace_right = update_teletype_text(wm, h, 1, cw.bio_birthplace_entry_right, &o_rank, tp.birthplace_right);
        tp.strategy_right = update_teletype_text(wm, h, 1, cw.bio_strategy_entry_right, &o_strategy, tp.strategy_right);
        h.challenge_windows().text_pos = tp;
    }
}

/// `ChallengeLoadScreen::activatePiecesMinSpec(generalPlayer, generalOpponent)` (LoadScreen.cpp:906).
pub fn challenge_activate_pieces_min_spec<H: ChallengeLoadScreenHost>(wm: &mut GameWindowManager, h: &mut H, player: &GeneralPersonaInfo, opponent: &GeneralPersonaInfo) {
    let cw = h.challenge_windows().clone();
    hide_opt(wm, cw.bio_name_left, false);
    hide_opt(wm, cw.bio_birthplace_left, false);
    hide_opt(wm, cw.bio_strategy_left, false);
    hide_opt(wm, cw.bio_name_right, false);
    hide_opt(wm, cw.bio_birthplace_right, false);
    hide_opt(wm, cw.bio_strategy_right, false);
    for w in [cw.bio_big_name_entry_left, cw.bio_name_entry_left, cw.bio_birthplace_entry_left, cw.bio_strategy_entry_left] {
        hide_opt(wm, w, false);
    }
    let p_name = h.fetch_text(&player.bio_name);
    let p_rank = h.fetch_text(&player.bio_rank);
    let p_strategy = h.fetch_text(&player.bio_strategy);
    gadget::static_text_set_text(wm, h, cw.bio_big_name_entry_left, &p_name);
    gadget::static_text_set_text(wm, h, cw.bio_name_entry_left, &p_name);
    gadget::static_text_set_text(wm, h, cw.bio_birthplace_entry_left, &p_rank);
    gadget::static_text_set_text(wm, h, cw.bio_strategy_entry_left, &p_strategy);
    for w in [cw.bio_big_name_entry_right, cw.bio_name_entry_right, cw.bio_birthplace_entry_right, cw.bio_strategy_entry_right] {
        hide_opt(wm, w, false);
    }
    let o_name = h.fetch_text(&opponent.bio_name);
    let o_rank = h.fetch_text(&opponent.bio_rank);
    let o_strategy = h.fetch_text(&opponent.bio_strategy);
    gadget::static_text_set_text(wm, h, cw.bio_big_name_entry_right, &o_name);
    gadget::static_text_set_text(wm, h, cw.bio_name_entry_right, &o_name);
    gadget::static_text_set_text(wm, h, cw.bio_birthplace_entry_right, &o_rank);
    gadget::static_text_set_text(wm, h, cw.bio_strategy_entry_right, &o_strategy);
    if let Some(w) = cw.portrait_left {
        h.win_set_enabled_image_named(wm, w, 0, &player.bio_portrait_large);
    }
    if let Some(w) = cw.portrait_right {
        h.win_set_enabled_image_named(wm, w, 0, &opponent.bio_portrait_large);
    }
    hide_opt(wm, cw.portrait_left, false);
    hide_opt(wm, cw.portrait_right, false);
    hide_opt(wm, cw.overlay_reticle_circle_alpha_outer, false);
    hide_opt(wm, cw.overlay_reticle_circle_alpha_inner, false);
    hide_opt(wm, cw.overlay_vs_backdrop, false);
    hide_opt(wm, cw.overlay_vs_backdrop, false); // (upstream hides it twice)
    hide_opt(wm, cw.overlay_vs, false);
    let mut mgr = h.challenge_windows().wnd_video_manager.take().unwrap_or_default();
    if let Some(w) = cw.overlay_vs {
        mgr.play_movie(wm, h, w, b"VSSmall", WindowVideoPlayType::ShowLastFrame);
    }
    h.challenge_windows().wnd_video_manager = Some(mgr);
}

/// `ChallengeLoadScreen::init(game)` (LoadScreen.cpp:944).
pub fn challenge_init<H: ChallengeLoadScreenHost>(wm: &mut GameWindowManager, h: &mut H) {
    let mission = h.current_mission();

    // the player general is tied to the campaign
    let general_player = h.player_general_by_campaign_name(&mission.campaign_name);

    // the opponent general is tied to the mission
    let general_name = h.current_mission_general_name();
    let general_opponent = h.general_by_general_name(&general_name);

    // create the layout of the load screen
    let ls = h.win_create_from_script(wm, b"Menus/ChallengeLoadScreen.wnd");
    h.load_screen().load_screen = ls;
    let Some(ls) = ls else { return }; // (DEBUG_ASSERTCRASH: upstream dereferences NULL)
    wm.win_hide(ls, false);
    wm.win_bring_to_top(ls);

    // Store the pointer to the progress bar on the loadscreen
    let progress_bar = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:ProgressLoad");
    h.load_screen().progress_bar = progress_bar;
    if let Some(p) = progress_bar {
        gadget::progress_bar_set_progress(wm, h, p, 0);
    }

    // upstream: m_ambientLoop.setEventName("LoadScreenAmbient") — set in audio_add_ambient_loop
    // create the new background video stream and the new buffer
    let Some((stream, buffer)) = h.video_open_stream_and_buffer(&mission.movie_label) else {
        return;
    };
    h.load_screen().video_stream = Some(stream);
    h.load_screen().video_buffer = Some(buffer);

    // init overlays
    let portrait_left = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:PortraitLeft");
    let portrait_right = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:PortraitRight");
    let portrait_movie_left = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:PortraitMovieLeft");
    let portrait_movie_right = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:PortraitMovieRight");
    let circle_outer = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:CircleAlphaOuter");
    let circle_inner = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:CircleAlphaInner");
    let vs_backdrop = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:VersusBackdrop");
    let overlay_vs = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:OverlayVs");

    let bio_name_left = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:BioNameLeft");
    let bio_birthplace_left = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:BioBirthplaceLeft");
    let bio_strategy_left = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:BioStrategyLeft");
    let bio_big_name_entry_left = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:BigNameEntryLeft");
    let bio_name_entry_left = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:BioNameEntryLeft");
    let bio_birthplace_entry_left = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:BioBirthplaceEntryLeft");
    let bio_strategy_entry_left = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:BioStrategyEntryLeft");
    let bio_name_right = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:BioNameRight");
    let bio_birthplace_right = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:BioBirthplaceRight");
    let bio_strategy_right = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:BioStrategyRight");
    let bio_big_name_entry_right = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:BigNameEntryRight");
    let bio_name_entry_right = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:BioNameEntryRight");
    let bio_birthplace_entry_right = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:BioBirthplaceEntryRight");
    let bio_strategy_entry_right = win_in(wm, h, Some(ls), "ChallengeLoadScreen.wnd:BioStrategyEntryRight");

    // make sure reticle stuff starts out hidden
    hide_opt(wm, circle_outer, true);
    hide_opt(wm, circle_inner, true);
    hide_opt(wm, vs_backdrop, true);
    hide_opt(wm, overlay_vs, true);

    {
        let cw = h.challenge_windows();
        cw.portrait_left = portrait_left;
        cw.portrait_right = portrait_right;
        cw.portrait_movie_left = portrait_movie_left;
        cw.portrait_movie_right = portrait_movie_right;
        cw.overlay_reticle_circle_alpha_outer = circle_outer;
        cw.overlay_reticle_circle_alpha_inner = circle_inner;
        cw.overlay_vs_backdrop = vs_backdrop;
        cw.overlay_vs = overlay_vs;
        cw.bio_name_left = bio_name_left;
        cw.bio_birthplace_left = bio_birthplace_left;
        cw.bio_strategy_left = bio_strategy_left;
        cw.bio_big_name_entry_left = bio_big_name_entry_left;
        cw.bio_name_entry_left = bio_name_entry_left;
        cw.bio_birthplace_entry_left = bio_birthplace_entry_left;
        cw.bio_strategy_entry_left = bio_strategy_entry_left;
        cw.bio_name_right = bio_name_right;
        cw.bio_birthplace_right = bio_birthplace_right;
        cw.bio_strategy_right = bio_strategy_right;
        cw.bio_big_name_entry_right = bio_big_name_entry_right;
        cw.bio_name_entry_right = bio_name_entry_right;
        cw.bio_birthplace_entry_right = bio_birthplace_entry_right;
        cw.bio_strategy_entry_right = bio_strategy_entry_right;
        // m_wndVideoManager = NEW WindowVideoManager; m_wndVideoManager->init();
        let mut mgr = WindowVideoManager::new();
        mgr.init();
        cw.wnd_video_manager = Some(mgr);
    }

    if h.game_lod_did_mem_pass() {
        let progress_update_count = h.video_frame_count(stream) / FRAME_FUDGE_ADD;
        let mut shifted_percent = -FRAME_FUDGE_ADD + 1;
        while h.video_frame_index(stream) < h.video_frame_count(stream) - 1 {
            h.service_windows_os();

            if !h.video_is_frame_ready(stream) {
                h.sleep_ms(1);
                continue;
            }

            if !h.game_engine_is_active() {
                // we are alt-tabbed out, so just increment the frame
                // (frameNext() then frameDecompress(): the host's decompress-render does both with the render
                // skipped; the buffer is not drawn to while inactive)
                h.video_frame_next(stream);
                continue;
            }

            h.video_decompress_render(stream, buffer);
            h.video_frame_next(stream);

            h.win_set_video_buffer(wm, ls, Some(buffer));

            let frame = h.video_frame_index(stream);
            // (`% progressUpdateCount`: a zero count is a division by zero upstream; modelled as no update)
            if progress_update_count != 0 && frame % progress_update_count == 0 {
                shifted_percent += 1;
                if shifted_percent > 0 {
                    shifted_percent = 0;
                }
                let percent = ((shifted_percent + FRAME_FUDGE_ADD) as f64 / 1.3) as i32;
                h.mouse_clear_cursor_tooltip();
                if let Some(p) = progress_bar {
                    gadget::progress_bar_set_progress(wm, h, p, percent);
                }
            }
            wm.update(h);

            challenge_activate_pieces(wm, h, frame, &general_player, &general_opponent);
            let mut mgr = h.challenge_windows().wnd_video_manager.take().unwrap_or_default();
            mgr.update(wm, h);
            h.challenge_windows().wnd_video_manager = Some(mgr);

            // redraw all views, update the GUI
            h.display_draw();

            h.audio_update();
        }
    } else {
        // if we're min speced
        h.video_frame_goto_end(stream); // zero based
        while !h.video_is_frame_ready(stream) {
            h.sleep_ms(1);
        }
        h.video_decompress_render(stream, buffer);
        h.win_set_video_buffer(wm, ls, Some(buffer));

        challenge_activate_pieces_min_spec(wm, h, &general_player, &general_opponent);

        let delay = mission.voice_length * 1000;
        let begin = h.time_get_time() as i32;
        let mut curr_time = begin;
        while begin + delay > curr_time {
            let fudge_factor = (30.0 * ((curr_time - begin) as f32 / delay as f32)) as i32;
            if let Some(p) = progress_bar {
                gadget::progress_bar_set_progress(wm, h, p, fudge_factor);
            }

            wm.update(h);
            h.display_draw();
            h.sleep_ms(100);
            curr_time = h.time_get_time() as i32;
        }

        let mut mgr = h.challenge_windows().wnd_video_manager.take().unwrap_or_default();
        mgr.update(wm, h);
        h.challenge_windows().wnd_video_manager = Some(mgr);
        wm.update(h);
        h.display_draw();
    }
    h.set_fp_mode();

    let taunt = h.opponent_random_taunt_sound(&general_opponent);
    h.audio_add_event_named(&taunt);

    let handle = h.audio_add_ambient_loop();
    h.load_screen().ambient_loop_handle = handle;
    h.audio_update();
}

// MultiPlayerLoadScreen and GameSpyLoadScreen: constructors, destructors, init ///////////////////

/// The members `MultiPlayerLoadScreen` has beyond the shared [`LoadScreen`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MultiPlayerExtra {
    pub map_preview: Option<WinId>,
    pub button_map_start_position: [Option<WinId>; MAX_SLOTS],
    pub portrait_local_general: Option<WinId>,
    pub features_local_general: Option<WinId>,
    pub name_local_general: Option<WinId>,
}

/// The members `GameSpyLoadScreen` has beyond the shared [`LoadScreen`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GameSpyExtra {
    pub map_preview: Option<WinId>,
    pub button_map_start_position: [Option<WinId>; MAX_SLOTS],
    pub portrait_local_general: Option<WinId>,
    pub features_local_general: Option<WinId>,
    pub name_local_general: Option<WinId>,
    pub player_rank: [Option<WinId>; MAX_SLOTS],
    pub player_officer_medal: [Option<WinId>; MAX_SLOTS],
    pub player_favorite_factions: [Option<WinId>; MAX_SLOTS],
    pub player_total_disconnects: [Option<WinId>; MAX_SLOTS],
    pub player_win: [Option<WinId>; MAX_SLOTS],
    pub player_win_losses: [Option<WinId>; MAX_SLOTS],
}

/// What a game slot reports to the multiplayer load screens (`GameSlot`, `GameSpyGameSlot`).
#[derive(Clone, Debug, Default)]
pub struct LoadSlotInfo {
    pub occupied: bool,
    pub is_ai: bool,
    pub apparent_color: i32,
    pub name: Vec<u16>,
    /// `getApparentPlayerTemplateDisplayName()`.
    pub apparent_player_template_display_name: Vec<u16>,
    /// `getTeamNumber()`.
    pub team_number: i32,
    /// `getProfileID()` (GameSpy slots).
    pub profile_id: i32,
}

/// The local player's `PlayerTemplate` as the multiplayer load screens read it.
#[derive(Clone, Debug, Default)]
pub struct LocalTemplateInfo {
    /// `pt->getName()`, `getDisplayName()`, `getGeneralFeatures()` (a text label), `getLoadScreenMusic()`.
    pub name: Vec<u8>,
    pub display_name: Vec<u16>,
    pub general_features: Vec<u8>,
    pub load_screen_music: Vec<u8>,
}

/// The `PSPlayerStats` fields `GameSpyLoadScreen::init` sums (GameSpy persistent storage).
#[derive(Clone, Debug, Default)]
pub struct PlayerStatsInfo {
    pub id: i32,
    pub wins: Vec<i32>,
    pub losses: Vec<i32>,
    pub discons: Vec<i32>,
    pub desyncs: Vec<i32>,
}

/// What `MultiPlayerLoadScreen::init` and `GameSpyLoadScreen::init` reach beyond the base host.
pub trait MultiPlayerLoadScreenHost: LoadScreenHost {
    fn multi_player_extra(&mut self) -> &mut MultiPlayerExtra;
    fn game_spy_extra(&mut self) -> &mut GameSpyExtra;
    /// `game->getSlot(game->getLocalSlotNum())`'s template: `lSlot->getPlayerTemplate() >= 0` picks the Nth
    /// template, else `FactionObserver`.
    fn local_player_template(&self) -> LocalTemplateInfo;
    /// `TheChallengeGenerals->getGeneralByTemplateName(pt->getName())`, or `None` for the original factions.
    fn local_general_by_template_name(&self, template_name: &[u8]) -> Option<GeneralPersonaInfo>;
    /// `game->getSlot(i)` as `GameSlot` (`None` for no slot).
    fn load_slot(&self, i: usize) -> Option<LoadSlotInfo>;
    /// `win->winSetEnabledImage(index, TheMappedImageCollection->findImageByName(name))`; an empty name is `NULL`,
    /// a name not found is `NULL`.
    fn win_set_enabled_image_named_opt(&mut self, wm: &mut GameWindowManager, w: WinId, index: usize, name: &[u8]);
    /// Whether `TheMappedImageCollection->findImageByName(name)` finds the image.
    fn image_exists(&self, name: &[u8]) -> bool;
    /// `winSetEnabledTextColors(color, winGetEnabledTextBorderColor())`.
    fn win_set_text_color_keep_border(&mut self, wm: &mut GameWindowManager, w: WinId, color: Color);
    /// `TheMultiplayerSettings->getColor(slot->getApparentColor())->getColor()`.
    fn slot_house_color_by_color(&self, apparent_color: i32) -> Color;
    /// `TheAudio->removeAudioEvent(AHSV_StopTheMusicFade)`.
    fn audio_remove_stop_music_fade(&mut self);
    /// `AudioEventRTS event(name); event.setShouldFade(TRUE); TheAudio->addAudioEvent(&event); TheAudio->update()`.
    fn audio_add_event_fading_and_update(&mut self, name: &[u8]);
    /// The map preview block shared by both `init`s: `TheMapCache->findMap`, `winSetUserData(mmd)`,
    /// `getMapPreviewImage(game->getMap())`, `positionStartSpots(game, buttons, preview)`,
    /// `updateMapStartSpots(game, buttons, TRUE)` and the `WIN_STATUS_IMAGE` set/clear with the image.
    fn setup_map_preview(&mut self, wm: &mut GameWindowManager, map_preview: WinId, buttons: &[Option<WinId>; MAX_SLOTS]);
    /// `TheGameLogic->initTimeOutValues()`.
    fn game_logic_init_time_out_values(&mut self);
    /// `TheGameSpyPSMessageQueue->findPlayerStatsByID(slot->getProfileID())`.
    fn find_player_stats_by_id(&self, profile_id: i32) -> PlayerStatsInfo;
    /// `TheGameSpyInfo->didPlayerPreorder(id)`.
    fn did_player_preorder(&self, id: i32) -> bool;
    /// `CalculateRank(stats)`, `GetFavoriteSide(stats)`, `LookupSmallRankImage(favSide, rankPoints)` (an image name).
    fn lookup_small_rank_image_name(&self, stats: &PlayerStatsInfo) -> Vec<u8>;
    /// `GetAdditionalDisconnectsFromUserFile(playerID)`.
    fn additional_disconnects_from_user_file(&self, player_id: i32) -> i32;
}

/// `MultiPlayerLoadScreen::MultiPlayerLoadScreen()`.
pub fn multi_player_new<H: MultiPlayerLoadScreenHost>(h: &mut H) {
    *h.multi_player_extra() = MultiPlayerExtra::default();
    let l = h.load_screen();
    for i in 0..MAX_SLOTS {
        l.progress_bars[i] = None;
        l.player_names[i] = None;
        l.player_side[i] = None;
        l.player_lookup[i] = -1;
    }
}

/// `MultiPlayerLoadScreen::~MultiPlayerLoadScreen()`.
pub fn multi_player_destroy<H: MultiPlayerLoadScreenHost>(h: &mut H) {
    {
        let l = h.load_screen();
        for i in 0..MAX_SLOTS {
            l.progress_bars[i] = None;
            l.player_names[i] = None;
            l.player_side[i] = None;
            l.player_lookup[i] = -1;
        }
    }
    {
        let e = h.multi_player_extra();
        e.portrait_local_general = None;
        e.features_local_general = None;
        e.name_local_general = None;
    }
    h.audio_remove_stop_music_fade();
}

/// The local general block both `init`s share: portrait, features and name (`prefix` is the .wnd name).
/// The original factions have no general: their logo (`logos`, per faction) and the template's display name are used.
fn local_general_block<H: MultiPlayerLoadScreenHost>(wm: &mut GameWindowManager, h: &mut H, ls: WinId, prefix: &str, logos: [&[u8]; 3]) -> (Option<WinId>, Option<WinId>, Option<WinId>, LocalTemplateInfo) {
    let pt = h.local_player_template();
    let local_general = h.local_general_by_template_name(&pt.name);
    let (portrait_name, local_name): (Vec<u8>, Vec<u16>) = match local_general {
        Some(g) => (g.bio_portrait_large.clone(), h.fetch_text(&g.bio_name)),
        None => {
            // the main original factions don't have associated generals
            let portrait = match pt.name.as_slice() {
                b"FactionAmerica" => logos[0].to_vec(),
                b"FactionGLA" => logos[1].to_vec(),
                b"FactionChina" => logos[2].to_vec(),
                _ => Vec::new(), // DEBUG_ASSERTCRASH(NULL, ("Unexpected player template")); portrait stays NULL
            };
            (portrait, pt.display_name.clone())
        }
    };
    let portrait_win = win_in(wm, h, Some(ls), &format!("{prefix}:LocalGeneralPortrait"));
    if let Some(w) = portrait_win {
        h.win_set_enabled_image_named_opt(wm, w, 0, &portrait_name);
    }
    let features_win = win_in(wm, h, Some(ls), &format!("{prefix}:LocalGeneralFeatures"));
    let features: Vec<u8> = if pt.general_features.is_empty() { b"GUI:PlayerObserver".to_vec() } else { pt.general_features.clone() };
    let features_text = h.fetch_text(&features);
    gadget::static_text_set_text(wm, h, features_win, &features_text);
    let name_win = win_in(wm, h, Some(ls), &format!("{prefix}:LocalGeneralName"));
    gadget::static_text_set_text(wm, h, name_win, &local_name);
    (portrait_win, features_win, name_win, pt)
}

/// The house progress bar image: `LoadingBar_ProgressCenter%d`, or `LoadingBar_Progress` when it is not found.
fn house_bar_image_name<H: MultiPlayerLoadScreenHost>(h: &H, apparent_color: i32) -> Vec<u8> {
    let name = format!("LoadingBar_ProgressCenter{apparent_color}").into_bytes();
    if h.image_exists(&name) { name } else { b"LoadingBar_Progress".to_vec() }
}

/// `MultiPlayerLoadScreen::init(game)` (LoadScreen.cpp:1329).
pub fn multi_player_init<H: MultiPlayerLoadScreenHost>(wm: &mut GameWindowManager, h: &mut H) {
    // create the layout of the load screen
    let ls = h.win_create_from_script(wm, b"Menus/MultiplayerLoadScreen.wnd");
    h.load_screen().load_screen = ls;
    let Some(ls) = ls else { return }; // (DEBUG_ASSERTCRASH: upstream dereferences NULL)
    wm.win_hide(ls, false);
    wm.win_bring_to_top(ls);
    let map_preview = win_in(wm, h, Some(ls), "MultiplayerLoadScreen.wnd:WinMapPreview");
    h.multi_player_extra().map_preview = map_preview;

    let (portrait, features, name_win, pt) = local_general_block(
        wm, h, ls, "MultiplayerLoadScreen.wnd",
        [b"SAFactionLogoLg_US", b"SUFactionLogoLg_GLA", b"SNFactionLogoLg_China"],
    );
    {
        let e = h.multi_player_extra();
        e.portrait_local_general = portrait;
        e.features_local_general = features;
        e.name_local_general = name_win;
    }

    if !pt.load_screen_music.is_empty() {
        h.audio_remove_stop_music_fade();
        h.audio_add_event_fading_and_update(&pt.load_screen_music);
    }

    let mut team_win: [Option<WinId>; MAX_SLOTS] = [None; MAX_SLOTS];

    let mut net_slot = 0usize;
    // Loop through and make the loadscreen look all good.
    for i in 0..MAX_SLOTS {
        // Load the Progress Bar
        let bar = win_in(wm, h, Some(ls), &format!("MultiplayerLoadScreen.wnd:ProgressLoad{i}"));
        h.load_screen().progress_bars[i] = bar;
        // set the progressbar to zero
        if let Some(b) = bar {
            gadget::progress_bar_set_progress(wm, h, b, 0);
        }

        // Load MapStart Positions
        let btn = win_in(wm, h, Some(ls), &format!("MultiplayerLoadScreen.wnd:ButtonMapStartPosition{i}"));
        h.multi_player_extra().button_map_start_position[i] = btn;

        // Load the Player's name
        let name = win_in(wm, h, Some(ls), &format!("MultiplayerLoadScreen.wnd:StaticTextPlayer{i}"));
        h.load_screen().player_names[i] = name;

        // Load the Player's Side
        let side = win_in(wm, h, Some(ls), &format!("MultiplayerLoadScreen.wnd:StaticTextSide{i}"));
        h.load_screen().player_side[i] = side;

        team_win[i] = win_in(wm, h, Some(ls), &format!("MultiplayerLoadScreen.wnd:StaticTextTeam{i}"));

        // get the slot man!
        let Some(slot) = h.load_slot(i) else { continue };
        if !slot.occupied {
            continue;
        }

        let house_color = h.slot_house_color_by_color(slot.apparent_color);

        // format the progress bar to house colors
        let house_image = house_bar_image_name(h, slot.apparent_color);
        let l = h.load_screen().clone();
        if let Some(b) = l.progress_bars[net_slot] {
            h.win_set_enabled_image_named_opt(wm, b, 6, &house_image);
        }

        gadget::static_text_set_text(wm, h, l.player_names[net_slot], &slot.name);
        if let Some(w) = l.player_names[net_slot] {
            h.win_set_text_color_keep_border(wm, w, house_color);
        }

        gadget::static_text_set_text(wm, h, l.player_side[net_slot], &slot.apparent_player_template_display_name);
        if let Some(w) = l.player_side[net_slot] {
            h.win_set_text_color_keep_border(wm, w, house_color);
        }

        if slot.is_ai {
            hide_opt(wm, l.progress_bars[net_slot], true);
        }

        if let Some(t) = team_win[net_slot] {
            let team_str = format!("Team:{}", slot.team_number + 1);
            let text = h.fetch_text(team_str.as_bytes());
            gadget::static_text_set_text(wm, h, Some(t), &text);
            // (the border colour is read from m_playerNames[netSlot], as upstream does)
            h.win_set_text_color_keep_border(wm, t, house_color);
        }

        h.load_screen().player_lookup[i] = net_slot as i32; // save our mapping so we can update progress correctly

        net_slot += 1;
    }

    for i in net_slot..MAX_SLOTS {
        let l = h.load_screen().clone();
        hide_opt(wm, l.progress_bars[i], true);
        hide_opt(wm, l.player_names[i], true);
        hide_opt(wm, l.player_side[i], true);
        hide_opt(wm, team_win[i], true);
    }

    if let Some(mp) = map_preview {
        let buttons = h.multi_player_extra().button_map_start_position;
        h.setup_map_preview(wm, mp, &buttons);
    }

    h.game_logic_init_time_out_values();
}

/// `GameSpyLoadScreen::GameSpyLoadScreen()`.
pub fn game_spy_new<H: MultiPlayerLoadScreenHost>(h: &mut H) {
    *h.game_spy_extra() = GameSpyExtra::default();
    let l = h.load_screen();
    for i in 0..MAX_SLOTS {
        l.progress_bars[i] = None;
        l.player_names[i] = None;
        l.player_side[i] = None;
        l.player_lookup[i] = -1;
    }
}

/// `GameSpyLoadScreen::~GameSpyLoadScreen()`.
pub fn game_spy_destroy<H: MultiPlayerLoadScreenHost>(h: &mut H) {
    {
        let l = h.load_screen();
        for i in 0..MAX_SLOTS {
            l.progress_bars[i] = None;
            l.player_names[i] = None;
            l.player_side[i] = None;
            l.player_lookup[i] = -1;
        }
    }
    let e = h.game_spy_extra();
    for i in 0..MAX_SLOTS {
        e.player_favorite_factions[i] = None;
        e.player_total_disconnects[i] = None;
        e.player_win[i] = None;
        e.player_win_losses[i] = None;
    }
}

/// `GameSpyLoadScreen::init(game)` (LoadScreen.cpp:1580). The GameSpy stats come through the host
/// (`TheGameSpyPSMessageQueue`, `TheGameSpyInfo`: the service is gone, so the host supplies what it had).
pub fn game_spy_init<H: MultiPlayerLoadScreenHost>(wm: &mut GameWindowManager, h: &mut H) {
    // create the layout of the load screen
    let ls = h.win_create_from_script(wm, b"Menus/GameSpyLoadScreen.wnd");
    h.load_screen().load_screen = ls;
    let Some(ls) = ls else { return }; // (DEBUG_ASSERTCRASH: upstream dereferences NULL)
    wm.win_hide(ls, false);
    wm.win_bring_to_top(ls);
    let map_preview = win_in(wm, h, Some(ls), "GameSpyLoadScreen.wnd:WinMapPreview");
    h.game_spy_extra().map_preview = map_preview;

    let (portrait, features, name_win, _pt) = local_general_block(
        wm, h, ls, "GameSpyLoadScreen.wnd",
        [b"SAFactionLogo144_US", b"SUFactionLogo144_GLA", b"SNFactionLogo144_China"],
    );
    {
        let e = h.game_spy_extra();
        e.portrait_local_general = portrait;
        e.features_local_general = features;
        e.name_local_general = name_win;
    }

    let mut team_win: [Option<WinId>; MAX_SLOTS] = [None; MAX_SLOTS];

    let mut net_slot = 0usize;
    // Loop through and make the loadscreen look all good.
    for i in 0..MAX_SLOTS {
        let bar = win_in(wm, h, Some(ls), &format!("GameSpyLoadScreen.wnd:ProgressLoad{i}"));
        h.load_screen().progress_bars[i] = bar;
        if let Some(b) = bar {
            gadget::progress_bar_set_progress(wm, h, b, 0);
        }

        let name = win_in(wm, h, Some(ls), &format!("GameSpyLoadScreen.wnd:StaticTextPlayer{i}"));
        h.load_screen().player_names[i] = name;

        let btn = win_in(wm, h, Some(ls), &format!("GameSpyLoadScreen.wnd:ButtonMapStartPosition{i}"));
        let side = win_in(wm, h, Some(ls), &format!("GameSpyLoadScreen.wnd:StaticTextSide{i}"));
        h.load_screen().player_side[i] = side;
        let player_win = win_in(wm, h, Some(ls), &format!("GameSpyLoadScreen.wnd:WinPlayer{i}"));
        let total_disconnects = win_in(wm, h, Some(ls), &format!("GameSpyLoadScreen.wnd:StaticTextTotalDisconnects{i}"));
        // (the StaticTextFavoriteFaction lookup is commented out upstream)
        let win_losses = win_in(wm, h, Some(ls), &format!("GameSpyLoadScreen.wnd:StaticTextWinLoss{i}"));
        let rank = win_in(wm, h, Some(ls), &format!("GameSpyLoadScreen.wnd:WinRank{i}"));
        let officer = win_in(wm, h, Some(ls), &format!("GameSpyLoadScreen.wnd:WinOfficer{i}"));
        {
            let e = h.game_spy_extra();
            e.button_map_start_position[i] = btn;
            e.player_win[i] = player_win;
            e.player_total_disconnects[i] = total_disconnects;
            e.player_win_losses[i] = win_losses;
            e.player_rank[i] = rank;
            e.player_officer_medal[i] = officer;
        }

        // (the team window is looked up under the MultiplayerLoadScreen.wnd prefix upstream, as copied)
        team_win[i] = win_in(wm, h, Some(ls), &format!("MultiplayerLoadScreen.wnd:StaticTextTeam{i}"));

        // get the slot man!
        let Some(slot) = h.load_slot(i) else { continue };
        if !slot.occupied {
            continue;
        }

        let house_color = h.slot_house_color_by_color(slot.apparent_color);

        // format the progress bar to house colors
        let house_image = house_bar_image_name(h, slot.apparent_color);
        let l = h.load_screen().clone();
        if let Some(b) = l.progress_bars[net_slot] {
            h.win_set_enabled_image_named_opt(wm, b, 6, &house_image);
        }

        gadget::static_text_set_text(wm, h, l.player_names[net_slot], &slot.name);
        if let Some(w) = l.player_names[net_slot] {
            h.win_set_text_color_keep_border(wm, w, house_color);
        }

        // Get the stats for the player
        let stats = h.find_player_stats_by_id(slot.profile_id);

        let is_preorder = h.did_player_preorder(stats.id);
        let rank_img = h.lookup_small_rank_image_name(&stats);
        // (upstream indexes the medal and rank windows with `i`, not `netSlot`; kept)
        let e = h.game_spy_extra().clone();
        if let Some(w) = e.player_officer_medal[i] {
            let preorder_name: &[u8] = if is_preorder { b"OfficersClubsmall" } else { b"" };
            h.win_set_enabled_image_named_opt(wm, w, 0, preorder_name);
        }
        if let Some(w) = e.player_rank[i] {
            h.win_set_enabled_image_named_opt(wm, w, 0, &rank_img);
        }

        // pop wins and losses
        let num_losses: i32 = stats.losses.iter().sum();
        let num_wins: i32 = stats.wins.iter().sum();
        let fmt = utf16(&format!("{num_wins}/{num_losses}"));
        gadget::static_text_set_text(wm, h, e.player_win_losses[net_slot], &fmt);
        if let Some(w) = e.player_win_losses[net_slot] {
            h.win_set_text_color_keep_border(wm, w, house_color);
        }
        // (favoriteFaction: the games loop and its text are commented out upstream)
        // disconnects
        let mut num_games: i32 = stats.discons.iter().sum();
        num_games += stats.desyncs.iter().sum::<i32>();
        num_games += h.additional_disconnects_from_user_file(stats.id);

        let fmt = utf16(&format!("{num_games}"));
        gadget::static_text_set_text(wm, h, e.player_total_disconnects[net_slot], &fmt);
        if let Some(w) = e.player_total_disconnects[net_slot] {
            h.win_set_text_color_keep_border(wm, w, house_color);
        }
        gadget::static_text_set_text(wm, h, l.player_side[net_slot], &slot.apparent_player_template_display_name);
        if let Some(w) = l.player_side[net_slot] {
            h.win_set_text_color_keep_border(wm, w, house_color);
        }

        if slot.is_ai {
            hide_opt(wm, l.progress_bars[net_slot], true);
            hide_opt(wm, e.player_total_disconnects[net_slot], true);
            hide_opt(wm, e.player_win_losses[net_slot], true);
            hide_opt(wm, e.player_rank[net_slot], true);
            hide_opt(wm, e.player_officer_medal[net_slot], true);
        }

        if let Some(t) = team_win[net_slot] {
            let mut team_str = format!("Team:{}", slot.team_number + 1);
            if slot.is_ai && slot.team_number == -1 {
                team_str = "Team:AI".to_string();
            }
            let text = h.fetch_text(team_str.as_bytes());
            gadget::static_text_set_text(wm, h, Some(t), &text);
            h.win_set_text_color_keep_border(wm, t, house_color);
        }

        h.load_screen().player_lookup[i] = net_slot as i32; // save our mapping so we can update progress correctly

        net_slot += 1;
    }

    for i in net_slot..MAX_SLOTS {
        let e = h.game_spy_extra().clone();
        hide_opt(wm, e.player_win[i], true);
        // (the player name and side hides are commented out upstream)
    }

    if let Some(mp) = map_preview {
        let buttons = h.game_spy_extra().button_map_start_position;
        h.setup_map_preview(wm, mp, &buttons);
    }

    h.game_logic_init_time_out_values();
}
