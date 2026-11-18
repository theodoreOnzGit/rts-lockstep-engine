//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptActions.cpp (the calls on the client the actions end
//!     in; see `src/drafts/sweep_script/script_actions.rs` for the list and the stub each replaces)
//!
//! The client side of `ScriptClientCall`: [`apply_script_client_call`] makes the call the action made upstream. The view
//! calls go to [`ScriptedCamera`] (this directory's `w3d_view.rs`) and the existing `TacticalView`; the UI, display and
//! drawable calls go to [`ScriptClientHost`], whose methods are named for the upstream call they stand for. Their
//! implementations are the port's other client modules (`InGameUi`, `Display`/`cinematic_text`, `ControlBar`, `Radar`,
//! `Drawable`, `Eva`) and the draft sweeps of those classes; this file does not redefine them.
//!
//! Camera-lock follow, movement frames and shake advance in `ScriptedCamera::update`, once per client frame.

use rts_lockstep_engine::client::tactical_view::TacticalView;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::drafts::sweep_script::script_actions::ScriptClientCall;

use super::w3d_view::{CameraLockType, CameraShakeType, ScriptedCamera, ScriptedViewHost};

/// The non-camera client calls (`TheInGameUI`, `TheDisplay`, `TheControlBar`, `TheSnowManager`, `Drawable`, `TheEva`,
/// `TheGameEngine`, the global data's script overrides).
pub trait ScriptClientHost: ScriptedViewHost {
    /// `TheInGameUI->message(text)`.
    fn in_game_ui_message(&mut self, text: &[u8]);
    /// `TheInGameUI->playMovie(name)` (the radar's cameo movie).
    fn in_game_ui_play_movie(&mut self, name: &[u8]);
    /// `TheInGameUI->selectDrawable(drawable)`.
    fn in_game_ui_select_drawable(&mut self, id: i32);
    /// `TheInGameUI->setSuperweaponDisplayEnabledByScript(enabled)`.
    fn in_game_ui_set_superweapon_display_enabled(&mut self, enabled: bool);
    /// `TheInGameUI->hideObjectSuperweaponDisplayByScript(obj)`.
    fn in_game_ui_hide_object_superweapon_display(&mut self, id: i32);
    /// `TheInGameUI->showObjectSuperweaponDisplayByScript(obj)`.
    fn in_game_ui_show_object_superweapon_display(&mut self, id: i32);
    /// `TheInGameUI->addNamedTimer(name, text, countdown)`.
    fn in_game_ui_add_named_timer(&mut self, name: &[u8], text: &[u16], countdown: bool);
    /// `TheInGameUI->removeNamedTimer(name)`.
    fn in_game_ui_remove_named_timer(&mut self, name: &[u8]);
    /// `TheInGameUI->showNamedTimerDisplay(show)`.
    fn in_game_ui_show_named_timer_display(&mut self, show: bool);
    /// `TheDisplay->playMovie(name)`.
    fn display_play_movie(&mut self, name: &[u8]);
    /// `TheDisplay->setBorderShroudLevel(level)`.
    fn display_set_border_shroud_level(&mut self, level: u8);
    /// `TheDisplay->enableLetterBox(enable)`.
    fn display_enable_letter_box(&mut self, enable: bool);
    /// `HideControlBar(immediate)` (`hide`) or `ShowControlBar(immediate)`.
    fn control_bar_show(&mut self, hide: bool, immediate: bool);
    /// `TheControlBar->setFlash(on)`.
    fn control_bar_set_flash(&mut self, on: bool);
    /// `button->setFlashCount(count)` on the control bar's command button `button` (`ButtonH`).
    fn button_set_flash_count(&mut self, button: i32, count: i32);
    /// The global `doSkyBoxSet(draw)`.
    fn sky_box_set(&mut self, draw: bool);
    /// `TheSnowManager->setVisible(visible)`.
    fn snow_set_visible(&mut self, visible: bool);
    /// `TheWritableGlobalData->m_scriptOverrideInfantryLightScale = setting`.
    fn set_infantry_light_scale(&mut self, setting: f32);
    /// `drawable->setFlashColor(color)`.
    fn drawable_set_flash_color(&mut self, id: i32, color: i32);
    /// `drawable->setFlashCount(count)`.
    fn drawable_set_flash_count(&mut self, id: i32, count: i32);
    /// `drawable->setIndicatorColor(color)`.
    fn drawable_set_indicator_color(&mut self, id: i32, color: i32);
    /// `drawable->setEmoticon(name, frames)`.
    fn drawable_set_emoticon(&mut self, id: i32, name: &[u8], frames: i32);
    /// `TheEva->setEvaEnabled(enabled)`.
    fn eva_set_enabled(&mut self, enabled: bool);
    /// `TheGameEngine->setFramesPerSecondLimit(fps)`.
    fn engine_set_frames_per_second_limit(&mut self, fps: i32);
    /// `TheWritableGlobalData->m_useFpsLimit = use_limit`.
    fn set_use_fps_limit(&mut self, use_limit: bool);
}

/// Make one `ScriptClientCall` on the client, as the action upstream made it. `view` is the tactical view and `cam` the
/// scripted-camera state beside it. Returns nothing: no call here changes what the logic reads.
pub fn apply_script_client_call<H: ScriptClientHost>(call: &ScriptClientCall, cam: &mut ScriptedCamera, view: &mut TacticalView, h: &mut H) {
    use ScriptClientCall as C;
    match call {
        // ---- the tactical view ----
        C::ViewSetCameraLock { id } => cam.set_camera_lock(*id, h),
        C::ViewSnapToCameraLock => cam.snap_to_camera_lock(),
        C::ViewSetSnapMode { lock_type, lock_dist } => {
            let t = if *lock_type == 1 { CameraLockType::Tether } else { CameraLockType::Follow };
            cam.set_snap_mode(t, *lock_dist);
        }
        C::ViewMoveCameraTo { pos, milliseconds, shutter_ms, orient, ease_in, ease_out } => {
            cam.move_camera_to(view, pos, *milliseconds, *shutter_ms, *orient, *ease_in, *ease_out, h)
        }
        C::ViewMoveCameraAlongWaypointPath { locations, milliseconds, shutter_ms, orient, ease_in, ease_out } => {
            cam.move_camera_along_waypoint_path(view, locations, *milliseconds, *shutter_ms, *orient, *ease_in, *ease_out, h)
        }
        C::ViewZoomCamera { zoom, milliseconds, ease_in, ease_out } => cam.zoom_camera(view, *zoom, *milliseconds, *ease_in, *ease_out),
        C::ViewPitchCamera { pitch, milliseconds, ease_in, ease_out } => cam.pitch_camera(view, *pitch, *milliseconds, *ease_in, *ease_out),
        C::ViewRotateCamera { rotations, milliseconds, ease_in, ease_out } => cam.rotate_camera(view, *rotations, *milliseconds, *ease_in, *ease_out),
        C::ViewRotateCameraTowardObject { id, milliseconds, hold_milliseconds, ease_in, ease_out } => {
            cam.rotate_camera_toward_object(*id, *milliseconds, *hold_milliseconds, *ease_in, *ease_out)
        }
        C::ViewRotateCameraTowardPosition { pos, milliseconds, ease_in, ease_out, reverse } => {
            cam.rotate_camera_toward_position(view, pos, *milliseconds, *ease_in, *ease_out, *reverse)
        }
        C::ViewResetCamera { pos, milliseconds, ease_in, ease_out } => cam.reset_camera(view, pos, *milliseconds, *ease_in, *ease_out, h),
        C::ViewScrollBy { dx, dy } => {
            // `TheTacticalView->scrollBy(&offset)`
            let delta = rts_lockstep_engine::common::base_type::Coord2D { x: *dx, y: *dy };
            view.scroll_by(&delta, h);
        }
        C::ViewCameraModLookToward { pos } => cam.camera_mod_look_toward(view, pos, h),
        C::ViewCameraModFinalLookToward { pos } => cam.camera_mod_final_look_toward(pos),
        C::ViewCameraModFinalMoveTo { pos } => cam.camera_mod_final_move_to(pos),
        C::ViewCameraModFinalPitch { pitch, ease_in, ease_out } => cam.camera_mod_final_pitch(view, *pitch, *ease_in, *ease_out),
        C::ViewCameraModFinalZoom { zoom, ease_in, ease_out } => cam.camera_mod_final_zoom(view, *zoom, *ease_in, *ease_out, h),
        C::ViewCameraModFreeze { angle } => {
            if *angle {
                cam.camera_mod_freeze_angle(view);
            } else {
                cam.camera_mod_freeze_time();
            }
        }
        C::ViewCameraModFinalTimeMultiplier { multiplier } => cam.camera_mod_final_time_multiplier(*multiplier),
        C::ViewCameraModRollingAverage { frames } => cam.camera_mod_rolling_average(*frames),
        C::ViewSetTimeMultiplier { multiplier } => cam.set_time_multiplier(*multiplier),
        C::ViewLookAt { pos } => {
            // `W3DView::lookAt`: the position and transform are `TacticalView::look_at`'s, then the scripted movements stop.
            view.look_at(pos, h);
            cam.look_at_clears_movements();
        }
        C::ViewSetDefaultView { pitch, angle, max_height } => {
            let g = h.global().clone();
            view.set_default_view(*pitch, *angle, *max_height, &g);
        }
        C::ViewShake { pos, shake_type } => {
            let t = match *shake_type {
                0 => CameraShakeType::Subtle,
                1 => CameraShakeType::Normal,
                2 => CameraShakeType::Strong,
                3 => CameraShakeType::Severe,
                4 => CameraShakeType::CineExtreme,
                _ => CameraShakeType::CineInsane,
            };
            cam.shake(view, pos, t, h);
        }
        C::ViewAddCameraShake { pos, radius, duration, amplitude } => cam.add_camera_shake(pos, *radius, *duration, *amplitude, h),
        C::ViewCameraSlaveMode { slave } => match slave {
            Some((object, bone)) => cam.camera_enable_slave_mode(object, bone),
            None => cam.camera_disable_slave_mode(),
        },
        C::ViewSetViewFilter { filter } => {
            cam.set_view_filter(*filter);
        }
        C::ViewSetViewFilterMode { mode } => {
            cam.set_view_filter_mode(*mode);
        }
        C::ViewSetViewFilterPos { pos: _ } => {
            // upstream: ScreenMotionBlurFilter::setZoomToPos(pos) — device-only, not translated
        }
        C::ViewSetFadeParameters { frames: _, direction: _ } => {
            // upstream: ScreenBWFilter::setFadeParameters / ScreenCrossFadeFilter::setFadeParameters — device-only, not translated
        }
        C::ViewSetGuardBandBias { x, y } => cam.set_guard_band_bias(&rts_lockstep_engine::common::base_type::Coord2D { x: *x, y: *y }),

        // ---- the in-game UI ----
        C::UiMessage { text } => h.in_game_ui_message(text),
        C::UiPlayMovie { name } => h.in_game_ui_play_movie(name),
        C::UiSelectDrawable { id } => h.in_game_ui_select_drawable(*id),
        C::UiSetSuperweaponDisplayEnabled { enabled } => h.in_game_ui_set_superweapon_display_enabled(*enabled),
        C::UiObjectSuperweaponDisplay { id, hide } => {
            if *hide {
                h.in_game_ui_hide_object_superweapon_display(*id);
            } else {
                h.in_game_ui_show_object_superweapon_display(*id);
            }
        }
        C::UiAddNamedTimer { name, text, countdown } => h.in_game_ui_add_named_timer(name, text, *countdown),
        C::UiRemoveNamedTimer { name } => h.in_game_ui_remove_named_timer(name),
        C::UiShowNamedTimerDisplay { show } => h.in_game_ui_show_named_timer_display(*show),

        // ---- the display, the control bar, the world's look ----
        C::DisplayPlayMovie { name } => h.display_play_movie(name),
        C::DisplaySetBorderShroudLevel { level } => h.display_set_border_shroud_level(*level),
        C::DisplayEnableLetterBox { enable } => h.display_enable_letter_box(*enable),
        C::ControlBarShow { hide, immediate } => h.control_bar_show(*hide, *immediate),
        C::ControlBarSetFlash { on } => h.control_bar_set_flash(*on),
        C::ButtonSetFlashCount { button, count } => h.button_set_flash_count(*button, *count),
        C::SkyBoxSet { draw } => h.sky_box_set(*draw),
        C::SnowSetVisible { visible } => h.snow_set_visible(*visible),
        C::SetInfantryLightScale { setting } => h.set_infantry_light_scale(*setting),

        // ---- drawables, EVA, the engine ----
        C::DrawableSetFlashColor { id, color } => h.drawable_set_flash_color(*id, *color),
        C::DrawableSetFlashCount { id, count } => h.drawable_set_flash_count(*id, *count),
        C::DrawableSetIndicatorColor { id, color } => h.drawable_set_indicator_color(*id, *color),
        C::DrawableSetEmoticon { id, name, frames } => h.drawable_set_emoticon(*id, name, *frames),
        C::EvaSetEnabled { enabled } => h.eva_set_enabled(*enabled),
        C::EngineFpsLimit { fps, use_limit } => {
            if let Some(f) = fps {
                h.engine_set_frames_per_second_limit(*f);
            }
            if let Some(u) = use_limit {
                h.set_use_fps_limit(*u);
            }
        }
    }
}

/// `ScriptActions::doScreenShake(intensity)` (ScriptActions.cpp:5051): `getPosition(&pos); shake(&pos, intensity)`: the
/// epicentre is the view's own position, so the distance falloff is 0.
pub fn screen_shake_at_view_position(cam: &mut ScriptedCamera, view: &TacticalView, shake_type: CameraShakeType, h: &mut impl ScriptedViewHost) {
    let pos: Coord3D = view.pos;
    cam.shake(view, &pos, shake_type, h);
}
