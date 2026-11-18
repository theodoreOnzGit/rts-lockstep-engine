//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DView.cpp
//!
//! Sweep 2 (#213): the members of `W3DView` that the verified `rts_lockstep_engine::client::tactical_view::TacticalView`
//! does not have. Camera-in-motion state (`m_rcInfo`, `m_mcwpInfo`, `m_doingRotateCamera`, `m_freezeTimeForCameraMovement`,
//! `m_timeMultiplier`, `m_useRealZoomCam`, `m_nextWireFrameEnabled`, `m_guardBandBias`, `m_previousLookAtPosition`) lives in
//! [`W3DViewState`]; the functions that change `m_pos`/`m_angle`/`m_FXPitch`/`m_FOV` take the verified `TacticalView`.
//!
//! - `W3DView::drawView` (W3DView.cpp:1527) is `DRAW()`, the frame draw (device-only, not translated).
//! - `W3DView::setFieldOfView` calls `View::setFieldOfView`, then `setCameraTransform()` only in `_DEBUG`/`_INTERNAL`
//!   builds (not compiled in retail): drafted as the `View::setFieldOfView` store.
//! - `W3DView::screenToWorld` (W3DView.cpp:2071) is `DEBUG_CRASH(("implement me"))` upstream for a valid camera and
//!   does nothing otherwise: drafted as the no-op it is in a release build.

use rts_lockstep_engine::client::tactical_view::{norm_angle, TacticalView};
use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D, ICoord2D, Region3D};
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::ww::wwmath;

/// `DRAWABLE_OVERSCAN` (W3DView.cpp:110): 3D world coords of how much to overscan in the 3D screen region.
pub const DRAWABLE_OVERSCAN: f32 = 75.0;

/// `struct RotateCameraInfo` as `rotateCameraOneFrame` reads it.
#[derive(Clone, Copy, Debug, Default)]
pub struct RotateCameraInfo {
    pub num_frames: i32,
    pub cur_frame: i32,
    pub num_hold_frames: i32,
    pub track_object: bool,
    /// `target.targetObjectID` / `target.targetObjectPos`.
    pub target_object_id: ObjectId,
    pub target_object_pos: Coord3D,
    /// `angle.startAngle` / `angle.endAngle`.
    pub start_angle: f32,
    pub end_angle: f32,
    pub start_time_multiplier: i32,
    pub end_time_multiplier: i32,
}

/// The `W3DView` state the functions below use.
#[derive(Clone, Debug, Default)]
pub struct W3DViewState {
    pub rc_info: RotateCameraInfo,
    pub doing_rotate_camera: bool,
    pub freeze_time_for_camera_movement: bool,
    pub time_multiplier: i32,
    pub use_real_zoom_cam: bool,
    pub next_wire_frame_enabled: bool,
    /// `m_guardBandBias`.
    pub guard_band_bias: Coord2D,
    /// `m_previousLookAtPosition`.
    pub previous_look_at_position: Coord3D,
    /// `m_CameraArrivedAtWaypointOnPathFlag`.
    pub camera_arrived_at_waypoint_on_path_flag: bool,
    /// `TheGlobalData->m_disableCameraMovement`.
    pub disable_camera_movement: bool,
}

impl W3DViewState {
    /// `W3DView::getAxisAlignedViewRegion` (W3DView.cpp:1398): the bounding box of the four screen corners at z = 0, in
    /// world x/y, grown by `DRAWABLE_OVERSCAN` plus the guard band (60 more toward the top, y low side), with z spanning the
    /// map extent widened by 999999. `corners` is `getScreenCornerWorldPointsAtZ(.., 0.0)`, `map_extent` is
    /// `TheTerrainLogic->getExtent()`.
    pub fn get_axis_aligned_view_region(&self, corners: &[Coord3D; 4], map_extent: &Region3D) -> Region3D {
        let mut region = Region3D { lo: corners[0], hi: corners[0] };
        for c in corners.iter() {
            if c.x < region.lo.x {
                region.lo.x = c.x;
            }
            if c.y < region.lo.y {
                region.lo.y = c.y;
            }
            if c.x > region.hi.x {
                region.hi.x = c.x;
            }
            if c.y > region.hi.y {
                region.hi.y = c.y;
            }
        }
        let safe_value = 999999.0f32;
        region.lo.z = map_extent.lo.z - safe_value;
        region.hi.z = map_extent.hi.z + safe_value;
        region.lo.x -= DRAWABLE_OVERSCAN + self.guard_band_bias.x;
        region.lo.y -= DRAWABLE_OVERSCAN + self.guard_band_bias.y + 60.0;
        region.hi.x += DRAWABLE_OVERSCAN + self.guard_band_bias.x;
        region.hi.y += DRAWABLE_OVERSCAN + self.guard_band_bias.y;
        region
    }

    /// `W3DView::set3DWireFrameMode` (W3DView.cpp:1450): applied at the next frame.
    pub fn set_3d_wire_frame_mode(&mut self, enable: bool) {
        self.next_wire_frame_enabled = enable;
    }

    /// `W3DView::calcDeltaScroll` (W3DView.cpp:1502): how many pixels the look-at point moved on screen since last frame,
    /// for the motion blur; zero when either position is outside the frustum. `project(x, y, z)` is
    /// `m_3DCamera->Project(..)` returning the screen position only when the result is `INSIDE_FRUSTUM`.
    pub fn calc_delta_scroll(&self, pos: &Coord3D, ground_level: f32, project: &impl Fn(f32, f32, f32) -> Option<(f32, f32)>) -> Coord2D {
        let mut screen_delta = Coord2D { x: 0.0, y: 0.0 };
        let prev_screen = match project(self.previous_look_at_position.x, self.previous_look_at_position.y, ground_level) {
            Some(p) => p,
            None => return screen_delta,
        };
        let screen = match project(pos.x, pos.y, ground_level) {
            Some(p) => p,
            None => return screen_delta,
        };
        screen_delta.x = screen.0 - prev_screen.0;
        screen_delta.y = screen.1 - prev_screen.1;
        screen_delta
    }

    /// `W3DView::setFieldOfView` (W3DView.cpp:2005): `View::setFieldOfView(angle)`.
    pub fn set_field_of_view(view: &mut TacticalView, angle: f32) {
        view.fov = angle;
    }

    /// `W3DView::screenToWorld` (W3DView.cpp:2071): a release-build no-op (`DEBUG_CRASH("implement me")`).
    pub fn screen_to_world(_s: &ICoord2D) -> Option<Coord3D> {
        None
    }

    /// `W3DView::isCameraMovementAtWaypointAlongPath` (W3DView.cpp:2830): polls and clears the flag (for scripting).
    pub fn is_camera_movement_at_waypoint_along_path(&mut self) -> bool {
        let return_value = self.camera_arrived_at_waypoint_on_path_flag;
        self.camera_arrived_at_waypoint_on_path_flag = false;
        return_value
    }

    /// `W3DView::rotateCameraOneFrame` (W3DView.cpp:2975): advances a scripted camera rotation by one frame. `ease(t)` is
    /// `m_rcInfo.ease`; `find_object_pos(id)` is `TheGameLogic->findObjectByID(id)->getPosition()`.
    pub fn rotate_camera_one_frame(&mut self, view: &mut TacticalView, ease: &impl Fn(f32) -> f32, find_object_pos: &impl Fn(ObjectId) -> Option<Coord3D>) {
        self.rc_info.cur_frame += 1;
        if self.disable_camera_movement {
            if self.rc_info.cur_frame >= self.rc_info.num_frames + self.rc_info.num_hold_frames {
                self.doing_rotate_camera = false;
                self.freeze_time_for_camera_movement = false;
            }
            return;
        }
        if self.rc_info.track_object {
            if self.rc_info.cur_frame <= self.rc_info.num_frames + self.rc_info.num_hold_frames {
                if let Some(p) = find_object_pos(self.rc_info.target_object_id) {
                    self.rc_info.target_object_pos = p;
                }
                let dir = (self.rc_info.target_object_pos.x - view.pos.x, self.rc_info.target_object_pos.y - view.pos.y);
                let dir_length = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
                if dir_length >= 0.1 {
                    let mut angle = wwmath::acos(dir.0 / dir_length);
                    if dir.1 < 0.0 {
                        angle = -angle;
                    }
                    angle -= std::f32::consts::PI / 2.0;
                    angle = norm_angle(angle);
                    if self.rc_info.cur_frame <= self.rc_info.num_frames {
                        let factor = ease(self.rc_info.cur_frame as f32 / self.rc_info.num_frames as f32);
                        let mut angle_diff = angle - view.angle;
                        angle_diff = norm_angle(angle_diff);
                        angle_diff *= factor;
                        view.angle += angle_diff;
                        view.angle = norm_angle(view.angle);
                        self.time_multiplier = self.rc_info.start_time_multiplier
                            + ((0.5 + (self.rc_info.end_time_multiplier - self.rc_info.start_time_multiplier) as f64 * factor as f64).floor() as i32);
                    } else {
                        view.angle = angle;
                    }
                }
            }
        } else if self.rc_info.cur_frame <= self.rc_info.num_frames {
            let factor = ease(self.rc_info.cur_frame as f32 / self.rc_info.num_frames as f32);
            view.angle = wwmath::lerp(self.rc_info.start_angle, self.rc_info.end_angle, factor);
            view.angle = norm_angle(view.angle);
            self.time_multiplier = self.rc_info.start_time_multiplier
                + ((0.5 + (self.rc_info.end_time_multiplier - self.rc_info.start_time_multiplier) as f64 * factor as f64).floor() as i32);
        }
        if self.rc_info.cur_frame >= self.rc_info.num_frames + self.rc_info.num_hold_frames {
            self.doing_rotate_camera = false;
            self.freeze_time_for_camera_movement = false;
            if !self.rc_info.track_object {
                view.angle = self.rc_info.end_angle;
            }
        }
    }

    /// `W3DView::cameraEnableRealZoomMode` (W3DView.cpp:3334): `m_useRealZoomCam = true; m_FXPitch = 1; updateView()`.
    /// The caller runs `updateView()` after.
    pub fn camera_enable_real_zoom_mode(&mut self, view: &mut TacticalView) {
        self.use_real_zoom_cam = true;
        view.fx_pitch = 1.0; // Reset to default
    }

    /// `W3DView::cameraDisableRealZoomMode` (W3DView.cpp:3342): also resets the field of view to 50 degrees; the caller
    /// runs `setCameraTransform()` and `updateView()` after.
    pub fn camera_disable_real_zoom_mode(&mut self, view: &mut TacticalView) {
        self.use_real_zoom_cam = false;
        view.fx_pitch = 1.0; // Reset to default
        view.fov = 50.0 * std::f32::consts::PI / 180.0;
    }
}
