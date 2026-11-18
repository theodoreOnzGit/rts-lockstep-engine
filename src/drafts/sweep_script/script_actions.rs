//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptActions.cpp (the calls the actions make on
//!     `TheTacticalView`, `TheInGameUI`, `TheDisplay`, `TheControlBar`, `TheRadar`, `TheSnowManager`, `TheGameLogic`'s
//!     client options, `Drawable`)
//!
//! **What this is.** In the verified port the ~80 `sa_cut(..)` calls in `src/logic/world/script_actions_host_world.rs`
//! only log a `CutCall::Host { call, .. }`: the action's front end (argument parsing, name lookup, the `* 1000.0`
//! conversions) is translated in `script_actions.rs`, but the *call on the client* it ends in is not made. Some stubs also
//! push a `ClientEvent` (cinematic text, popup message, military subtitle, radar create/hide/force-on/refresh, the audio
//! and input calls); the rest do nothing. [`ScriptClientCall`] is the event those remaining stubs should push, one
//! variant per stub, in the order the action makes it ("the client never changes the simulation": nothing here is read,
//! saved or CRC'd by the logic). The client side that makes each call is
//! `client/src/drafts/sweep_script/script_actions.rs` (`apply_script_client_call`) and, for the camera,
//! `client/src/drafts/sweep_script/w3d_view.rs`.
//!
//! **Wiring (not done; verified files are not edited by the sweep).** `ClientEvent` gets one variant
//! `ScriptClient(ScriptClientCall)`; each stub below becomes `self.events.push(ClientEvent::ScriptClient(..))` beside (or in
//! place of) its `sa_cut`, exactly as `sa_ui_popup_message` does for `InGameUiCall::PopupMessage`.
//!
//! **Stubs left alone, with the reason:**
//! - `sa_append_debug_message` (`ScriptEngine::AppendDebugMessage`): the Windows `DebugWindow.dll` (loaded by
//!   `GetProcAddress`), not translated (`script_engine.rs` header).
//! - `sa_create_crater_in_terrain`, `sa_partition_*`, `sa_partition_garrison_candidates`,
//!   `sa_partition_unit_garrison_candidates`, `sa_partition_nearest_group_with_value`,
//!   `sa_partition_most_valuable_target`, `sa_refresh_shroud_for_local_player`: logic-side. The terrain and partition
//!   have live implementations behind `cuts.live_terrain` / `cuts.live_partition`; the `sa_cut` is the flag-off fallback
//!   only, and there is nothing to draft.
//! - `sa_win_create_from_script`, `sa_win_destroy` (`TheWindowManager->winCreateFromScript` / `winDestroy`): the port's
//!   `client::window_script` is the client side; the logic keeps a window handle only.
//! - `sa_adjust_font_size`: already corrected in the port (the client applies `adjustFontSize` to the event's size).

use crate::common::base_type::Coord3D;
use crate::common::game_type::ObjectId;

/// One of the script actions' calls on the client that the verified port logs and does not make. Each variant names the
/// stub it replaces (`World::sa_*` in `src/logic/world/script_actions_host_world.rs`), the action that makes the call
/// (`ScriptActions::do*`, ScriptActions.cpp) and the upstream call.
#[derive(Clone, Debug, PartialEq)]
pub enum ScriptClientCall {
    // ---- the tactical view (`TheTacticalView`; milliseconds as the action passes them) ----
    /// `sa_view_set_camera_lock`: `TheTacticalView->setCameraLock(id)` (`doCameraTetherNamed`, `doCameraFollowNamed`,
    /// `doCameraStopTetherNamed`, `doCameraStopFollow`).
    ViewSetCameraLock { id: ObjectId },
    /// `sa_view_snap_to_camera_lock`: `TheTacticalView->snapToCameraLock()`.
    ViewSnapToCameraLock,
    /// `sa_view_set_snap_mode`: `TheTacticalView->setSnapMode(lockType, lockDist)` (`View::LOCK_TETHER` for
    /// `doCameraTetherNamed`; `lock_type` 0 is `LOCK_FOLLOW`, 1 `LOCK_TETHER`).
    ViewSetSnapMode { lock_type: i32, lock_dist: f32 },
    /// `sa_view_move_camera_to` (`doMoveCameraTo`, `doSetupCamera`, `doCameraMoveHome`).
    ViewMoveCameraTo { pos: Coord3D, milliseconds: i32, shutter_ms: i32, orient: bool, ease_in: f32, ease_out: f32 },
    /// `sa_view_move_camera_along_waypoint_path` (`doMoveCameraAlongWaypointPath`): the chain's locations in `getLink(0)`
    /// order, from the named waypoint (the client has no `Waypoint`).
    ViewMoveCameraAlongWaypointPath { locations: Vec<Coord3D>, milliseconds: i32, shutter_ms: i32, orient: bool, ease_in: f32, ease_out: f32 },
    /// `sa_view_zoom_camera` (`doZoomCamera`).
    ViewZoomCamera { zoom: f32, milliseconds: i32, ease_in: f32, ease_out: f32 },
    /// `sa_view_pitch_camera` (`doPitchCamera`).
    ViewPitchCamera { pitch: f32, milliseconds: i32, ease_in: f32, ease_out: f32 },
    /// `sa_view_rotate_camera` (`doRotateCamera`).
    ViewRotateCamera { rotations: f32, milliseconds: i32, ease_in: f32, ease_out: f32 },
    /// `sa_view_rotate_camera_toward_object` (`doRotateCameraTowardObject`).
    ViewRotateCameraTowardObject { id: ObjectId, milliseconds: i32, hold_milliseconds: i32, ease_in: f32, ease_out: f32 },
    /// `sa_view_rotate_camera_toward_position` (`doRotateCameraTowardWaypoint`).
    ViewRotateCameraTowardPosition { pos: Coord3D, milliseconds: i32, ease_in: f32, ease_out: f32, reverse: bool },
    /// `sa_view_reset_camera` (`doResetCamera`).
    ViewResetCamera { pos: Coord3D, milliseconds: i32, ease_in: f32, ease_out: f32 },
    /// `sa_view_scroll_by` (`doOversizeTheTerrain`'s two `scrollBy(+-0.0001)`).
    ViewScrollBy { dx: f32, dy: f32 },
    /// `sa_view_camera_mod_look_toward` (`doModCameraLookToward`, `doSetupCamera`).
    ViewCameraModLookToward { pos: Coord3D },
    /// `sa_view_camera_mod_final_look_toward` (`doModCameraFinalLookToward`).
    ViewCameraModFinalLookToward { pos: Coord3D },
    /// `sa_view_camera_mod_final_move_to` (`doModCameraFinalMoveTo`).
    ViewCameraModFinalMoveTo { pos: Coord3D },
    /// `sa_view_camera_mod_final_pitch` (`doModCameraFinalPitch`, `doSetupCamera`).
    ViewCameraModFinalPitch { pitch: f32, ease_in: f32, ease_out: f32 },
    /// `sa_view_camera_mod_final_zoom` (`doModCameraFinalZoom`, `doSetupCamera`).
    ViewCameraModFinalZoom { zoom: f32, ease_in: f32, ease_out: f32 },
    /// `sa_view_camera_mod_freeze`: `cameraModFreezeTime()` or `cameraModFreezeAngle()` (`doCameraModFreezeTime`,
    /// `doCameraModFreezeAngle`).
    ViewCameraModFreeze { angle: bool },
    /// `sa_view_camera_mod_final_time_multiplier` (`doCameraModFinalTimeMultiplier`).
    ViewCameraModFinalTimeMultiplier { multiplier: i32 },
    /// `sa_view_camera_mod_rolling_average` (`doCameraModRollingAverage`).
    ViewCameraModRollingAverage { frames: i32 },
    /// `sa_view_set_time_multiplier` (`doSetTimeMultiplier`, `doCameraModSetTimeMultiplier`).
    ViewSetTimeMultiplier { multiplier: i32 },
    /// `sa_view_look_at` (`doMoveCameraToSelection`, `doCameraMoveHome`, ...): `TheTacticalView->lookAt(pos)`.
    ViewLookAt { pos: Coord3D },
    /// `sa_view_set_default_view` (`doCameraSetDefault`): `setDefaultView(pitch, angle, maxHeight)`.
    ViewSetDefaultView { pitch: f32, angle: f32, max_height: f32 },
    /// `sa_view_shake` (`doScreenShake`: at the view's own position): `shake(&pos, (CameraShakeType)intensity)`.
    ViewShake { pos: Coord3D, shake_type: i32 },
    /// `sa_view_add_camera_shake` (`doC3CameraShake`): `Add_Camera_Shake(position, radius, duration, power)`.
    ViewAddCameraShake { pos: Coord3D, radius: f32, duration: f32, amplitude: f32 },
    /// `sa_view_camera_slave_mode` (`doC3CameraEnableSlaveMode` / `doC3CameraDisableSlaveMode`).
    ViewCameraSlaveMode { slave: Option<(Vec<u8>, Vec<u8>)> },
    /// `sa_view_set_view_filter` (`doBlackWhiteMode`): `setViewFilter(filter)`.
    ViewSetViewFilter { filter: i32 },
    /// `sa_view_set_view_filter_mode`: `setViewFilterMode(mode)`.
    ViewSetViewFilterMode { mode: i32 },
    /// `sa_view_set_view_filter_pos`: `setViewFilterPos(&pos)`.
    ViewSetViewFilterPos { pos: Coord3D },
    /// `sa_view_set_fade_parameters` (`doBlackWhiteMode`, `doFadeIn`...): `setFadeParameters(frames, direction)`.
    ViewSetFadeParameters { frames: i32, direction: i32 },
    /// `sa_view_set_guard_band_bias` (`doResizeViewGuardband`): `setGuardBandBias(&{gbx, gby})`.
    ViewSetGuardBandBias { x: f32, y: f32 },

    // ---- the in-game UI (`TheInGameUI`) ----
    /// `sa_ui_message` (`doDisplayText`): `TheInGameUI->message(displayText)`.
    UiMessage { text: Vec<u8> },
    /// `sa_ui_play_movie` (`doMoviePlayRadar`): `TheInGameUI->playMovie(name)`.
    UiPlayMovie { name: Vec<u8> },
    /// `sa_ui_select_drawable` (`doSelectUnit`...): `TheInGameUI->selectDrawable(drawable)`.
    UiSelectDrawable { id: ObjectId },
    /// `sa_ui_set_superweapon_display_enabled` (`doDisableSpecialPowerDisplay` / `doEnableSpecialPowerDisplay`):
    /// `setSuperweaponDisplayEnabledByScript(enabled)`.
    UiSetSuperweaponDisplayEnabled { enabled: bool },
    /// `sa_ui_object_superweapon_display` (`doNamedHideSpecialPowerDisplay` / `doNamedShowSpecialPowerDisplay`):
    /// `hideObjectSuperweaponDisplayByScript(obj)` (`hide`) or `showObjectSuperweaponDisplayByScript(obj)`.
    UiObjectSuperweaponDisplay { id: ObjectId, hide: bool },
    /// `sa_ui_add_named_timer` (`doDisplayCounter` with `countdown` false, `doDisplayCountdownTimer` with true):
    /// `addNamedTimer(name, TheGameText->fetch(text), countdown)`; `text` is the fetched UTF-16 string.
    UiAddNamedTimer { name: Vec<u8>, text: Vec<u16>, countdown: bool },
    /// `sa_ui_remove_named_timer` (`doHideCounter`, `doHideCountdownTimer`).
    UiRemoveNamedTimer { name: Vec<u8> },
    /// `sa_ui_show_named_timer_display` (`doDisableCountdownTimerDisplay` false / `doEnableCountdownTimerDisplay` true).
    UiShowNamedTimerDisplay { show: bool },

    // ---- the display, the control bar, the world's look ----
    /// `sa_display_play_movie` (`doMoviePlayFullScreen`): `TheDisplay->playMovie(name)`.
    DisplayPlayMovie { name: Vec<u8> },
    /// `sa_display_set_border_shroud_level` (`doSetBorderShroud`): `TheDisplay->setBorderShroudLevel(shroudAlpha or
    /// clearAlpha)`; the logic has already chosen the level.
    DisplaySetBorderShroudLevel { level: u8 },
    /// `sa_display_enable_letter_box` (`doLetterBoxMode`'s `TheDisplay->enableLetterBox`).
    DisplayEnableLetterBox { enable: bool },
    /// `sa_control_bar_show` (`doLetterBoxMode`'s `HideControlBar(immediate)` / `ShowControlBar(immediate)`).
    ControlBarShow { hide: bool, immediate: bool },
    /// `sa_control_bar_set_flash` (`doFlashControlBar`...): `TheControlBar->setFlash(on)`.
    ControlBarSetFlash { on: bool },
    /// `sa_button_set_flash_count` (`doFlashButton`, ScriptActions.cpp:2639): `button->setFlashCount(count)`, then
    /// `TheControlBar->setFlash(TRUE)` (the latter is the following `ControlBarSetFlash`).
    ButtonSetFlashCount { button: i32, count: i32 },
    /// `sa_sky_box_set` (`doSkyBox`): the global `doSkyBoxSet(startDraw)`.
    SkyBoxSet { draw: bool },
    /// `sa_snow_set_visible` (`doWeather`): `TheSnowManager->setVisible(showWeather)`.
    SnowSetVisible { visible: bool },
    /// `sa_set_infantry_light_scale`: the *client's* read of `TheWritableGlobalData->m_scriptOverrideInfantryLightScale`
    /// (the logic stores the global; the drawable lighting reads it).
    SetInfantryLightScale { setting: f32 },

    // ---- drawables and the radar ----
    /// `sa_drawable_set_flash_color` (`doNamedFlash`, `doTeamFlash`, ScriptActions.cpp:2689, 2721): `setFlashColor(color)`.
    DrawableSetFlashColor { id: ObjectId, color: i32 },
    /// `sa_drawable_set_flash_count` (same actions): `setFlashCount(count)`.
    DrawableSetFlashCount { id: ObjectId, count: i32 },
    /// `sa_drawable_set_indicator_color` (ScriptActions.cpp:114): `draw->setIndicatorColor(color)`.
    DrawableSetIndicatorColor { id: ObjectId, color: i32 },
    /// `sa_drawable_set_emoticon` (`doNamedEmoticon`, `doTeamEmoticon`): `draw->setEmoticon(name, frames)`.
    DrawableSetEmoticon { id: ObjectId, name: Vec<u8>, frames: i32 },
    /// `sa_eva_set_enabled` (`doEvaEnableDisable`): `TheEva->setEvaEnabled(enabled)`.
    EvaSetEnabled { enabled: bool },
    /// `sa_engine_set_fps_limit` / `sa_set_use_fps_limit` (`doSetFPSLimit`): `TheGameEngine->setFramesPerSecondLimit(fps)` /
    /// `TheGlobalData->m_useFpsLimit`.
    EngineFpsLimit { fps: Option<i32>, use_limit: Option<bool> },
}
