//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DView.cpp (the scripted camera: `W3DView::W3DView`'s
//!     members, `reset`, `updateCameraMovements`, `update`, `setCameraLock`, `setSnapMode`, `lookAt`'s flag clears,
//!     `moveCameraTo`, `rotateCamera`, `rotateCameraTowardObject`, `rotateCameraTowardPosition`, `zoomCamera`,
//!     `pitchCamera`, `cameraMod*`, `resetCamera`, `isCameraMovementFinished`, `isCameraMovementAtWaypointAlongPath`,
//!     `moveCameraAlongWaypointPath`, `setupWaypointPath`, `rotateCameraOneFrame`, `zoomCameraOneFrame`,
//!     `pitchCameraOneFrame`, `moveAlongWaypointPath`, `shake`, `cameraEnableSlaveMode`, `cameraDisableSlaveMode`,
//!     `cameraEnableRealZoomMode`, `cameraDisableRealZoomMode`, `Add_Camera_Shake`, `setViewFilter*`, `setFadeParameters`)
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DView.h (`TMoveAlongWaypointPathInfo`,
//!     `TRotateCameraInfo`, `TPitchCameraInfo`, `TZoomCameraInfo`, the members)
//!   GeneralsMD/Code/GameEngine/Include/GameClient/View.h (`setCameraLock`, `snapToCameraLock`, `setSnapMode`,
//!     `cameraModFreezeTime`, `isTimeFrozen`, `getTimeMultiplier`, `setTimeMultiplier`, `setGuardBandBias`)
//!
//! **What this replaces.** The `sa_view_*` stubs in `src/logic/world/script_actions_host_world.rs`
//! (`sa_view_set_camera_lock`, `sa_view_snap_to_camera_lock`, `sa_view_set_snap_mode`, `sa_view_move_camera_to`,
//! `sa_view_move_camera_along_waypoint_path`, `sa_view_zoom_camera`, `sa_view_pitch_camera`, `sa_view_rotate_camera`,
//! `sa_view_rotate_camera_toward_object`, `sa_view_rotate_camera_toward_position`, `sa_view_reset_camera`,
//! `sa_view_camera_mod_*`, `sa_view_set_time_multiplier`, `sa_view_shake`, `sa_view_add_camera_shake`,
//! `sa_view_camera_slave_mode`, `sa_view_set_view_filter*`, `sa_view_set_guard_band_bias`, `sa_view_look_at`), which
//! each `sa_cut(..)` today. `crate::client::tactical_view::TacticalView` documents the scripted camera as "not
//! translated"; this is that part. It is client-only state: the logic reads none of it ("the client never changes the
//! simulation"), except `isCameraMovementFinished` / `isCameraMovementAtWaypointAlongPath`, which the conditions ask
//! (`TheTacticalView->isCameraMovementFinished()` through `cuts.game_loop.view_camera_movement_finished` today).
//!
//! **Representation.** [`ScriptedCamera`] holds the `W3DView`/`View` members the scripts drive; [`TacticalView`] keeps
//! the ones it already has (`m_pos`, `m_angle`, `m_zoom`, `m_FXPitch`, `m_groundLevel`, `m_cameraOffset`,
//! `m_cameraConstraint`). Every method takes the view and a [`ScriptedViewHost`]. `TRotateCameraInfo`'s `union { Target;
//! Angle; }` is two separate fields here (the bytes a stale other member would leave are undefined upstream and never
//! read on a defined path). `TheW3DFrameLengthInMsec` is its default, 33 (`1000/LOGICFRAMES_PER_SECOND`).
//!
//! **Device-only, not translated:** `W3DShaderManager::filterSetup` (`setViewFilter*` always succeed here),
//! `ScreenBWFilter`/`ScreenCrossFadeFilter::setFadeParameters`, `ScreenMotionBlurFilter::setZoomToPos`, the terrain
//! render object and `drawDrawable` (the end of `update`), `clientOnly_getFirstRenderObjInfo` (the camera-lock-on-a-
//! drawable branch: the host returns it through [`ScriptedViewHost::locked_drawable_camera`]).

use rts_lockstep_engine::client::tactical_view::{get_height_around_pos, norm_angle, TacticalView, ViewHost};
use rts_lockstep_engine::common::base_type::{real_to_int_floor, Coord2D, Coord3D, PI};
use rts_lockstep_engine::common::game_common::LOGICFRAMES_PER_SECOND;
use rts_lockstep_engine::common::game_type::{ObjectId, INVALID_ID};
use rts_lockstep_engine::drafts::logic_common::parabolic_ease::ParabolicEase;
use rts_lockstep_engine::ww::wwmath::{acos, lerp};

use crate::drafts::device::camerashakesystem::CameraShakeSystemClass;
use rts_lockstep_engine::ww::vector3::Vector3;

/// `TheW3DFrameLengthInMsec` (W3DView.cpp:109): `1000/LOGICFRAMES_PER_SECOND`, 33 ms.
pub const THE_W3D_FRAME_LENGTH_IN_MSEC: i32 = 1000 / LOGICFRAMES_PER_SECOND;
/// `MAX_WAYPOINTS` (W3DView.h:52).
pub const MAX_WAYPOINTS: usize = 25;
/// `MAP_XY_FACTOR`.
const MAP_XY_FACTOR: f32 = 10.0;
/// `PATHFIND_CELL_SIZE_F`.
const PATHFIND_CELL_SIZE_F: f32 = 10.0;

/// `View::CameraLockType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraLockType {
    /// `LOCK_FOLLOW`.
    Follow,
    /// `LOCK_TETHER`.
    Tether,
}

/// `CameraShakeType` (View.h): the scripts' shake sizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraShakeType {
    /// `SHAKE_SUBTLE`.
    Subtle,
    /// `SHAKE_NORMAL`.
    Normal,
    /// `SHAKE_STRONG`.
    Strong,
    /// `SHAKE_SEVERE`.
    Severe,
    /// `SHAKE_CINE_EXTREME`.
    CineExtreme,
    /// `SHAKE_CINE_INSANE`.
    CineInsane,
}

/// `TMoveAlongWaypointPathInfo` (W3DView.h:56).
#[derive(Clone, Copy, Debug)]
pub struct MoveAlongWaypointPathInfo {
    /// `numWaypoints`.
    pub num_waypoints: i32,
    /// `waypoints[MAX_WAYPOINTS+2]` ("we pad first & last for interpolation").
    pub waypoints: [Coord3D; MAX_WAYPOINTS + 2],
    /// `waySegLength[MAX_WAYPOINTS+2]`.
    pub way_seg_length: [f32; MAX_WAYPOINTS + 2],
    /// `cameraAngle[MAX_WAYPOINTS+2]`.
    pub camera_angle: [f32; MAX_WAYPOINTS + 2],
    /// `timeMultiplier[MAX_WAYPOINTS+2]`.
    pub time_multiplier: [i32; MAX_WAYPOINTS + 2],
    /// `groundHeight[MAX_WAYPOINTS+1]`.
    pub ground_height: [f32; MAX_WAYPOINTS + 1],
    /// `totalTimeMilliseconds`.
    pub total_time_milliseconds: i32,
    /// `elapsedTimeMilliseconds`.
    pub elapsed_time_milliseconds: i32,
    /// `totalDistance`.
    pub total_distance: f32,
    /// `curSegDistance`.
    pub cur_seg_distance: f32,
    /// `shutter`.
    pub shutter: i32,
    /// `curSegment`.
    pub cur_segment: i32,
    /// `curShutter`.
    pub cur_shutter: i32,
    /// `rollingAverageFrames`.
    pub rolling_average_frames: i32,
    /// `ease`.
    pub ease: ParabolicEase,
}

impl Default for MoveAlongWaypointPathInfo {
    fn default() -> Self {
        MoveAlongWaypointPathInfo {
            num_waypoints: 0,
            waypoints: [Coord3D::default(); MAX_WAYPOINTS + 2],
            way_seg_length: [0.0; MAX_WAYPOINTS + 2],
            camera_angle: [0.0; MAX_WAYPOINTS + 2],
            time_multiplier: [0; MAX_WAYPOINTS + 2],
            ground_height: [0.0; MAX_WAYPOINTS + 1],
            total_time_milliseconds: 0,
            elapsed_time_milliseconds: 0,
            total_distance: 0.0,
            cur_seg_distance: 0.0,
            shutter: 0,
            cur_segment: 0,
            cur_shutter: 0,
            rolling_average_frames: 0,
            ease: ParabolicEase::new(),
        }
    }
}

/// `TRotateCameraInfo` (W3DView.h:77).
#[derive(Clone, Copy, Debug)]
pub struct RotateCameraInfo {
    /// `numFrames`.
    pub num_frames: i32,
    /// `curFrame`.
    pub cur_frame: i32,
    /// `startTimeMultiplier`.
    pub start_time_multiplier: i32,
    /// `endTimeMultiplier`.
    pub end_time_multiplier: i32,
    /// `numHoldFrames`.
    pub num_hold_frames: i32,
    /// `ease`.
    pub ease: ParabolicEase,
    /// `trackObject`.
    pub track_object: bool,
    /// `target.targetObjectID` (the union's first member).
    pub target_object_id: ObjectId,
    /// `target.targetObjectPos`.
    pub target_object_pos: Coord3D,
    /// `angle.startAngle` (the union's second member).
    pub start_angle: f32,
    /// `angle.endAngle`.
    pub end_angle: f32,
}

impl Default for RotateCameraInfo {
    fn default() -> Self {
        RotateCameraInfo {
            num_frames: 0,
            cur_frame: 0,
            start_time_multiplier: 0,
            end_time_multiplier: 0,
            num_hold_frames: 0,
            ease: ParabolicEase::new(),
            track_object: false,
            target_object_id: INVALID_ID,
            target_object_pos: Coord3D::default(),
            start_angle: 0.0,
            end_angle: 0.0,
        }
    }
}

/// `TPitchCameraInfo` (W3DView.h:102).
#[derive(Clone, Copy, Debug)]
pub struct PitchCameraInfo {
    /// `numFrames`.
    pub num_frames: i32,
    /// `curFrame`.
    pub cur_frame: i32,
    /// `startPitch`.
    pub start_pitch: f32,
    /// `endPitch`.
    pub end_pitch: f32,
    /// `startTimeMultiplier`.
    pub start_time_multiplier: i32,
    /// `endTimeMultiplier`.
    pub end_time_multiplier: i32,
    /// `ease`.
    pub ease: ParabolicEase,
}

impl Default for PitchCameraInfo {
    fn default() -> Self {
        PitchCameraInfo { num_frames: 0, cur_frame: 0, start_pitch: 0.0, end_pitch: 0.0, start_time_multiplier: 0, end_time_multiplier: 0, ease: ParabolicEase::new() }
    }
}

/// `TZoomCameraInfo` (W3DView.h:116).
#[derive(Clone, Copy, Debug)]
pub struct ZoomCameraInfo {
    /// `numFrames`.
    pub num_frames: i32,
    /// `curFrame`.
    pub cur_frame: i32,
    /// `startZoom`.
    pub start_zoom: f32,
    /// `endZoom`.
    pub end_zoom: f32,
    /// `startTimeMultiplier`.
    pub start_time_multiplier: i32,
    /// `endTimeMultiplier`.
    pub end_time_multiplier: i32,
    /// `ease`.
    pub ease: ParabolicEase,
}

impl Default for ZoomCameraInfo {
    fn default() -> Self {
        ZoomCameraInfo { num_frames: 0, cur_frame: 0, start_zoom: 0.0, end_zoom: 0.0, start_time_multiplier: 0, end_time_multiplier: 0, ease: ParabolicEase::new() }
    }
}

/// What a camera locked on an object needs from the world (`TheGameLogic->findObjectByID(cameraLock)`).
#[derive(Clone, Copy, Debug)]
pub struct LockedObject {
    /// `getPosition()`.
    pub position: Coord3D,
    /// `isUsingAirborneLocomotor() && isAboveTerrainOrWater()`.
    pub airborne_above_terrain: bool,
    /// `getOrientation()`.
    pub orientation: f32,
}

/// What the scripted camera asks of the engine, beside the [`ViewHost`] questions the tactical view already asks.
pub trait ScriptedViewHost: ViewHost {
    /// `TheGameLogic->findObjectByID(id)`, with what `W3DView::update` reads of it; `None` if it is gone.
    fn locked_object(&self, id: ObjectId) -> Option<LockedObject>;
    /// `TheGameLogic->findObjectByID(id)->getPosition()` for `rotateCameraOneFrame`.
    fn object_position(&self, id: ObjectId) -> Option<Coord3D>;
    /// The camera-lock-on-a-drawable branch's `clientOnly_getFirstRenderObjInfo` and `Look_At`: `true` if the drawable's
    /// render object placed the camera (then `recalcCamera = false`). Device-only; the default is "no drawable locked".
    fn locked_drawable_camera(&mut self) -> bool {
        false
    }
    /// `TheScriptEngine->isTimeFrozenDebug()`.
    fn time_frozen_debug(&self) -> bool;
    /// `TheScriptEngine->isTimeFrozenScript()`.
    fn time_frozen_script(&self) -> bool;
    /// `TheGameLogic->isInReplayGame()`.
    fn in_replay_game(&self) -> bool;
    /// `TheInGameUI->isScrolling()`.
    fn ui_scrolling(&self) -> bool;
    /// `TheScriptEngine->isTimeFast()`.
    fn time_fast(&self) -> bool;
    /// `GameClientRandomValueReal(lo, hi)`.
    fn game_client_random_value_real(&mut self, lo: f32, hi: f32) -> f32;
    /// `WWMath::Random_Float()`: the unit-interval draw the camera shaker system takes.
    fn unit_random(&mut self) -> f32;
}

/// `W3DView`'s scripted-camera members (and `View`'s lock and time members).
#[derive(Clone, Debug)]
pub struct ScriptedCamera {
    /// `m_mcwpInfo`.
    pub mcwp_info: MoveAlongWaypointPathInfo,
    /// `m_doingMoveCameraOnWaypointPath`.
    pub doing_move_camera_on_waypoint_path: bool,
    /// `m_CameraArrivedAtWaypointOnPathFlag`.
    pub camera_arrived_at_waypoint_on_path_flag: bool,
    /// `m_rcInfo`.
    pub rc_info: RotateCameraInfo,
    /// `m_doingRotateCamera`.
    pub doing_rotate_camera: bool,
    /// `m_pcInfo`.
    pub pc_info: PitchCameraInfo,
    /// `m_doingPitchCamera`.
    pub doing_pitch_camera: bool,
    /// `m_zcInfo`.
    pub zc_info: ZoomCameraInfo,
    /// `m_doingZoomCamera`.
    pub doing_zoom_camera: bool,
    /// `m_doingScriptedCameraLock`.
    pub doing_scripted_camera_lock: bool,
    /// `m_freezeTimeForCameraMovement`.
    pub freeze_time_for_camera_movement: bool,
    /// `m_timeMultiplier`.
    pub time_multiplier: i32,
    /// `m_previousLookAtPosition`.
    pub previous_look_at_position: Coord3D,
    /// `View::m_cameraLock`.
    pub camera_lock: ObjectId,
    /// `View::m_lockType`.
    pub lock_type: CameraLockType,
    /// `View::m_lockDist`.
    pub lock_dist: f32,
    /// `View::m_snapImmediate`.
    pub snap_immediate: bool,
    /// `update`'s function-local `static Real followFactor = -1`.
    pub follow_factor: f32,
    /// `m_shakeOffset`.
    pub shake_offset: Coord2D,
    /// `m_shakeAngleCos`, `m_shakeAngleSin`, `m_shakeIntensity`.
    pub shake_angle_cos: f32,
    pub shake_angle_sin: f32,
    pub shake_intensity: f32,
    /// `CameraShakerSystem` (the global `CameraShakeSystemClass`).
    pub camera_shaker_system: CameraShakeSystemClass,
    /// `m_viewFilterMode`, `m_viewFilter` (`FM_VIEW_DEFAULT`, `FT_VIEW_DEFAULT` = their enum values; kept as numbers).
    pub view_filter_mode: i32,
    pub view_filter: i32,
    /// `m_guardBandBias`.
    pub guard_band_bias: Coord2D,
    /// `m_isCameraSlaved`, `m_cameraSlaveObjectName`, `m_cameraSlaveObjectBoneName`.
    pub is_camera_slaved: bool,
    pub camera_slave_object_name: Vec<u8>,
    pub camera_slave_object_bone_name: Vec<u8>,
    /// `m_useRealZoomCam`.
    pub use_real_zoom_cam: bool,
}

/// `FM_VIEW_MB_IN_AND_OUT_ALPHA` and its siblings, `FT_VIEW_MOTION_BLUR_FILTER` (W3DShaderManager.h enums): the numbers
/// `isCameraMovementFinished` tests. Not re-derived here: the shader manager's enum order is `FT_NULL_FILTER = 0`,
/// `FT_VIEW_DEFAULT`, `FT_VIEW_BW_FILTER`, `FT_VIEW_MOTION_BLUR_FILTER`, `FT_MAX`, and the motion-blur modes follow
/// `FM_VIEW_MB_IN_AND_OUT_ALPHA`, `..._SATURATE`, `FM_VIEW_MB_IN_ALPHA`, `FM_VIEW_MB_OUT_ALPHA`, `FM_VIEW_MB_IN_SATURATE`,
/// `FM_VIEW_MB_OUT_SATURATE`. The caller supplies them so a wrong guess here cannot hide.
#[derive(Clone, Copy, Debug)]
pub struct MotionBlurEnums {
    /// `FT_VIEW_MOTION_BLUR_FILTER`.
    pub motion_blur_filter: i32,
    /// The six `FM_VIEW_MB_*` in/out modes `isCameraMovementFinished` lists.
    pub in_out_modes: [i32; 6],
}

/// `makeQuadraticS` is declared `static` upstream and its only call is commented out; it is not translated.

/// Angle of a direction as the scripted camera takes it (the repeated block in `rotateCameraTowardPosition`,
/// `cameraModLookToward`, `cameraModFinalLookToward`, `setupWaypointPath`): `acos(dir.X/len)`, negated for `dir.Y < 0`, less
/// a quarter turn ("default camera is rotated 90 degrees, so match"), folded by `normAngle`.
fn direction_angle(dx: f32, dy: f32, len: f32) -> f32 {
    let mut angle = acos(dx / len);
    if dy < 0.0 {
        angle = -angle;
    }
    angle -= PI / 2.0;
    norm_angle(angle)
}

/// `Vector2::Length()`.
fn length2(dx: f32, dy: f32) -> f32 {
    rts_lockstep_engine::ww::wwmath::sqrt(dx * dx + dy * dy)
}

/// The mid-point blend `moveAlongWaypointPath` and the `cameraMod*LookToward` pair share: from `start`, `mid`, `end`
/// (already halved) and `factor`, `result = start + factor*(end-start) + (1-factor)*factor*((mid-end)+(mid-start))`.
fn blend(start: &Coord3D, mid: &Coord3D, end: &Coord3D, factor: f32) -> Coord3D {
    let mut result = *start;
    result.x += factor * (end.x - start.x);
    result.y += factor * (end.y - start.y);
    result.x += (1.0 - factor) * factor * (mid.x - end.x + mid.x - start.x);
    result.y += (1.0 - factor) * factor * (mid.y - end.y + mid.y - start.y);
    result.z = 0.0;
    result
}

/// `(a + b) / 2` on x and y (upstream's `start.x += b.x; start.x /= 2`), z from `a`.
fn midpoint_xy(a: &Coord3D, b: &Coord3D) -> Coord3D {
    let mut s = *a;
    s.x += b.x;
    s.y += b.y;
    s.x /= 2.0;
    s.y /= 2.0;
    s
}

impl ScriptedCamera {
    /// `W3DView::W3DView()` / `View::View()` / `View::init()` for these members.
    pub fn new() -> Self {
        ScriptedCamera {
            mcwp_info: MoveAlongWaypointPathInfo::default(),
            doing_move_camera_on_waypoint_path: false,
            camera_arrived_at_waypoint_on_path_flag: false,
            rc_info: RotateCameraInfo::default(),
            doing_rotate_camera: false,
            pc_info: PitchCameraInfo::default(),
            doing_pitch_camera: false,
            zc_info: ZoomCameraInfo::default(),
            doing_zoom_camera: false,
            doing_scripted_camera_lock: false,
            freeze_time_for_camera_movement: false,
            // (View.h declares `m_timeMultiplier`; `W3DView::reset` sets it to 1 and a new game resets the view)
            time_multiplier: 1,
            previous_look_at_position: Coord3D::default(),
            camera_lock: INVALID_ID,
            lock_type: CameraLockType::Follow,
            lock_dist: 0.0,
            snap_immediate: false,
            follow_factor: -1.0,
            shake_offset: Coord2D { x: 0.0, y: 0.0 },
            shake_angle_cos: 0.0,
            shake_angle_sin: 0.0,
            shake_intensity: 0.0,
            camera_shaker_system: CameraShakeSystemClass::default(),
            view_filter_mode: 0,
            view_filter: 0,
            guard_band_bias: Coord2D { x: 0.0, y: 0.0 },
            is_camera_slaved: false,
            camera_slave_object_name: Vec::new(),
            camera_slave_object_bone_name: Vec::new(),
            use_real_zoom_cam: false,
        }
    }

    /// `W3DView::reset()` (W3DView.cpp:680): time rate back to 1, the camera to (0,0,0) in one millisecond, the default
    /// view filter and a zero guard band. (`View::reset` sets `m_zoomLimited = TRUE`, done on the view.)
    pub fn reset(&mut self, view: &mut TacticalView, h: &impl ScriptedViewHost) {
        view.zoom_limited = true;
        self.set_time_multiplier(1);
        let arbitrary_pos = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
        self.reset_camera(view, &arbitrary_pos, 1, 0.0, 0.0, h);
        // setViewFilter(FT_VIEW_DEFAULT)
        self.view_filter = 1;
        self.set_guard_band_bias(&Coord2D { x: 0.0, y: 0.0 });
    }

    /// `View::getTimeMultiplier()`.
    pub fn get_time_multiplier(&self) -> i32 {
        self.time_multiplier
    }

    /// `View::setTimeMultiplier(multiple)`.
    pub fn set_time_multiplier(&mut self, multiple: i32) {
        self.time_multiplier = multiple;
    }

    /// `View::cameraModFreezeTime()`.
    pub fn camera_mod_freeze_time(&mut self) {
        self.freeze_time_for_camera_movement = true;
    }

    /// `View::isTimeFrozen()`.
    pub fn is_time_frozen(&self) -> bool {
        self.freeze_time_for_camera_movement
    }

    /// `View::setGuardBandBias(gb)`.
    pub fn set_guard_band_bias(&mut self, gb: &Coord2D) {
        self.guard_band_bias.x = gb.x;
        self.guard_band_bias.y = gb.y;
    }

    /// `View::getCameraLock()`.
    pub fn get_camera_lock(&self) -> ObjectId {
        self.camera_lock
    }

    /// `W3DView::setCameraLock(id)` (W3DView.cpp:1781): not while camera movement is disabled; then `View::setCameraLock`
    /// (the lock, `m_lockDist = 0`, `LOCK_FOLLOW`) and `m_doingScriptedCameraLock = FALSE`.
    pub fn set_camera_lock(&mut self, id: ObjectId, h: &impl ViewHost) {
        // If we're disabling camera movements, don't lock onto the object.
        if h.global().disable_camera_movement && id != INVALID_ID {
            return;
        }
        self.camera_lock = id;
        self.lock_dist = 0.0;
        self.lock_type = CameraLockType::Follow;
        self.doing_scripted_camera_lock = false;
    }

    /// `View::snapToCameraLock()`.
    pub fn snap_to_camera_lock(&mut self) {
        self.snap_immediate = true;
    }

    /// `W3DView::setSnapMode(lockType, lockDist)` (W3DView.cpp:1793).
    pub fn set_snap_mode(&mut self, lock_type: CameraLockType, lock_dist: f32) {
        self.lock_type = lock_type;
        self.lock_dist = lock_dist;
        self.doing_scripted_camera_lock = true;
    }

    /// `W3DView::lookAt`'s tail (W3DView.cpp:2366): the scripted movements stop. The position and transform are
    /// `TacticalView::look_at`'s; call it first.
    pub fn look_at_clears_movements(&mut self) {
        self.doing_rotate_camera = false;
        self.doing_move_camera_on_waypoint_path = false;
        self.camera_arrived_at_waypoint_on_path_flag = false;
        self.doing_scripted_camera_lock = false;
    }

    /// `W3DView::moveCameraTo(o, milliseconds, shutter, orient, easeIn, easeOut)` (W3DView.cpp:2398).
    pub fn move_camera_to(&mut self, view: &mut TacticalView, o: &Coord3D, milliseconds: i32, _shutter: i32, orient: bool, ease_in: f32, ease_out: f32, h: &impl ViewHost) {
        let mut milliseconds = milliseconds;
        self.mcwp_info.waypoints[0] = view.pos;
        self.mcwp_info.camera_angle[0] = view.angle;
        self.mcwp_info.way_seg_length[0] = 0.0;

        self.mcwp_info.waypoints[1] = view.pos;
        self.mcwp_info.way_seg_length[1] = 0.0;

        self.mcwp_info.waypoints[2] = *o;
        self.mcwp_info.way_seg_length[2] = 0.0;

        self.mcwp_info.num_waypoints = 2;
        if milliseconds < 1 {
            milliseconds = 1;
        }
        self.mcwp_info.total_time_milliseconds = milliseconds;
        self.mcwp_info.shutter = 1;
        self.mcwp_info.ease.set_ease_times(ease_in / milliseconds as f32, ease_out / milliseconds as f32);
        self.mcwp_info.cur_segment = 1;
        self.mcwp_info.cur_seg_distance = 0.0;
        self.mcwp_info.total_distance = 0.0;

        self.setup_waypoint_path(view, orient, h);
        if self.mcwp_info.total_time_milliseconds == 1 {
            // do it instantly.
            self.move_along_waypoint_path(view, 1, h);
            self.doing_move_camera_on_waypoint_path = true;
            self.camera_arrived_at_waypoint_on_path_flag = false;
        }
    }

    /// `W3DView::rotateCamera(rotations, milliseconds, easeIn, easeOut)` (W3DView.cpp:2431).
    pub fn rotate_camera(&mut self, view: &TacticalView, rotations: f32, milliseconds: i32, ease_in: f32, ease_out: f32) {
        let mut milliseconds = milliseconds;
        self.rc_info.num_hold_frames = 0;
        self.rc_info.track_object = false;

        if milliseconds < 1 {
            milliseconds = 1;
        }
        self.rc_info.num_frames = milliseconds / THE_W3D_FRAME_LENGTH_IN_MSEC;
        if self.rc_info.num_frames < 1 {
            self.rc_info.num_frames = 1;
        }
        self.rc_info.cur_frame = 0;
        self.doing_rotate_camera = true;
        self.rc_info.start_angle = view.angle;
        self.rc_info.end_angle = view.angle + 2.0 * PI * rotations;
        self.rc_info.start_time_multiplier = self.time_multiplier;
        self.rc_info.end_time_multiplier = self.time_multiplier;
        self.rc_info.ease.set_ease_times(ease_in / milliseconds as f32, ease_out / milliseconds as f32);

        self.doing_move_camera_on_waypoint_path = false;
        self.camera_arrived_at_waypoint_on_path_flag = false;
    }

    /// `W3DView::rotateCameraTowardObject(id, milliseconds, holdMilliseconds, easeIn, easeOut)` (W3DView.cpp:2456).
    pub fn rotate_camera_toward_object(&mut self, id: ObjectId, milliseconds: i32, hold_milliseconds: i32, ease_in: f32, ease_out: f32) {
        let mut milliseconds = milliseconds;
        let mut hold_milliseconds = hold_milliseconds;
        self.rc_info.track_object = true;
        if hold_milliseconds < 1 {
            hold_milliseconds = 0;
        }
        self.rc_info.num_hold_frames = hold_milliseconds / THE_W3D_FRAME_LENGTH_IN_MSEC;
        if self.rc_info.num_hold_frames < 1 {
            self.rc_info.num_hold_frames = 0;
        }

        if milliseconds < 1 {
            milliseconds = 1;
        }
        self.rc_info.num_frames = milliseconds / THE_W3D_FRAME_LENGTH_IN_MSEC;
        if self.rc_info.num_frames < 1 {
            self.rc_info.num_frames = 1;
        }
        self.rc_info.cur_frame = 0;
        self.doing_rotate_camera = true;
        self.rc_info.target_object_id = id;
        self.rc_info.start_time_multiplier = self.time_multiplier;
        self.rc_info.end_time_multiplier = self.time_multiplier;
        self.rc_info.ease.set_ease_times(ease_in / milliseconds as f32, ease_out / milliseconds as f32);

        self.doing_move_camera_on_waypoint_path = false;
        self.camera_arrived_at_waypoint_on_path_flag = false;
    }

    /// `W3DView::rotateCameraTowardPosition(pLoc, milliseconds, easeIn, easeOut, reverseRotation)` (W3DView.cpp:2484).
    /// Upstream returns early, after the `numHoldFrames`, `trackObject` and `numFrames` writes, for a position within 0.1
    /// of the view's; that is kept.
    pub fn rotate_camera_toward_position(&mut self, view: &TacticalView, p_loc: &Coord3D, milliseconds: i32, ease_in: f32, ease_out: f32, reverse_rotation: bool) {
        let mut milliseconds = milliseconds;
        self.rc_info.num_hold_frames = 0;
        self.rc_info.track_object = false;

        if milliseconds < 1 {
            milliseconds = 1;
        }
        self.rc_info.num_frames = milliseconds / THE_W3D_FRAME_LENGTH_IN_MSEC;
        if self.rc_info.num_frames < 1 {
            self.rc_info.num_frames = 1;
        }
        let cur_pos = view.pos;
        let (dx, dy) = (p_loc.x - cur_pos.x, p_loc.y - cur_pos.y);
        let dir_length = length2(dx, dy);
        if dir_length < 0.1 {
            return;
        }
        let mut angle = direction_angle(dx, dy, dir_length);

        if reverse_rotation {
            if view.angle < angle {
                angle -= 2.0 * PI;
            } else {
                angle += 2.0 * PI;
            }
        }

        self.rc_info.cur_frame = 0;
        self.doing_rotate_camera = true;
        self.rc_info.start_angle = view.angle;
        self.rc_info.end_angle = angle;
        self.rc_info.start_time_multiplier = self.time_multiplier;
        self.rc_info.end_time_multiplier = self.time_multiplier;
        self.rc_info.ease.set_ease_times(ease_in / milliseconds as f32, ease_out / milliseconds as f32);

        self.doing_move_camera_on_waypoint_path = false;
        self.camera_arrived_at_waypoint_on_path_flag = false;
    }

    /// `W3DView::zoomCamera(finalZoom, milliseconds, easeIn, easeOut)` (W3DView.cpp:2528).
    pub fn zoom_camera(&mut self, view: &TacticalView, final_zoom: f32, milliseconds: i32, ease_in: f32, ease_out: f32) {
        let mut milliseconds = milliseconds;
        if milliseconds < 1 {
            milliseconds = 1;
        }
        self.zc_info.num_frames = milliseconds / THE_W3D_FRAME_LENGTH_IN_MSEC;
        if self.zc_info.num_frames < 1 {
            self.zc_info.num_frames = 1;
        }
        self.zc_info.cur_frame = 0;
        self.doing_zoom_camera = true;
        self.zc_info.start_zoom = view.zoom;
        self.zc_info.end_zoom = final_zoom;
        self.zc_info.ease.set_ease_times(ease_in / milliseconds as f32, ease_out / milliseconds as f32);
    }

    /// `W3DView::pitchCamera(finalPitch, milliseconds, easeIn, easeOut)` (W3DView.cpp:2544).
    pub fn pitch_camera(&mut self, view: &TacticalView, final_pitch: f32, milliseconds: i32, ease_in: f32, ease_out: f32) {
        let mut milliseconds = milliseconds;
        if milliseconds < 1 {
            milliseconds = 1;
        }
        self.pc_info.num_frames = milliseconds / THE_W3D_FRAME_LENGTH_IN_MSEC;
        if self.pc_info.num_frames < 1 {
            self.pc_info.num_frames = 1;
        }
        self.pc_info.cur_frame = 0;
        self.doing_pitch_camera = true;
        self.pc_info.start_pitch = view.fx_pitch;
        self.pc_info.end_pitch = final_pitch;
        self.pc_info.ease.set_ease_times(ease_in / milliseconds as f32, ease_out / milliseconds as f32);
    }

    /// `W3DView::cameraModFinalZoom(finalZoom, easeIn, easeOut)` (W3DView.cpp:2561). `zoomCamera`'s `Int milliseconds`
    /// takes the `Real time` truncated.
    pub fn camera_mod_final_zoom(&mut self, view: &TacticalView, final_zoom: f32, ease_in: f32, ease_out: f32, h: &impl ViewHost) {
        if self.doing_rotate_camera {
            let terrain_height_max = get_height_around_pos(h, view.pos.x, view.pos.y);
            let max_height = terrain_height_max + view.max_height_above_ground;
            let max_zoom = max_height / view.camera_offset.z;

            let time = ((self.rc_info.num_frames + self.rc_info.num_hold_frames - self.rc_info.cur_frame) * THE_W3D_FRAME_LENGTH_IN_MSEC) as f32;
            self.zoom_camera(view, final_zoom * max_zoom, time as i32, time * ease_in, time * ease_out);
        }
        if self.doing_move_camera_on_waypoint_path {
            let pos = self.mcwp_info.waypoints[self.mcwp_info.num_waypoints as usize];
            let terrain_height_max = get_height_around_pos(h, pos.x, pos.y);
            let max_height = terrain_height_max + view.max_height_above_ground;
            let max_zoom = max_height / view.camera_offset.z;

            let time = (self.mcwp_info.total_time_milliseconds - self.mcwp_info.elapsed_time_milliseconds) as f32;
            self.zoom_camera(view, final_zoom * max_zoom, time as i32, time * ease_in, time * ease_out);
        }
    }

    /// `W3DView::cameraModFreezeAngle()` (W3DView.cpp:2588).
    pub fn camera_mod_freeze_angle(&mut self, view: &TacticalView) {
        if self.doing_rotate_camera {
            if self.rc_info.track_object {
                self.rc_info.target_object_id = INVALID_ID;
            } else {
                self.rc_info.start_angle = view.angle;
                self.rc_info.end_angle = view.angle; // Silly, but consistent.
            }
        }
        if self.doing_move_camera_on_waypoint_path {
            for i in 0..self.mcwp_info.num_waypoints as usize {
                self.mcwp_info.camera_angle[i + 1] = self.mcwp_info.camera_angle[0];
            }
        }
    }

    /// `W3DView::cameraModLookToward(pLoc)` (W3DView.cpp:2609).
    pub fn camera_mod_look_toward(&mut self, view: &mut TacticalView, p_loc: &Coord3D, h: &impl ViewHost) {
        if self.doing_rotate_camera {
            return; // Doesn't apply to rotate about a point.
        }
        if self.doing_move_camera_on_waypoint_path {
            let mut i = 2usize;
            while i <= self.mcwp_info.num_waypoints as usize {
                let factor = 0.5f32;
                let start = midpoint_xy(&self.mcwp_info.waypoints[i - 1], &self.mcwp_info.waypoints[i]);
                let mid = self.mcwp_info.waypoints[i];
                let end = midpoint_xy(&self.mcwp_info.waypoints[i], &self.mcwp_info.waypoints[i + 1]);
                let result = blend(&start, &mid, &end, factor);
                let (dx, dy) = (p_loc.x - result.x, p_loc.y - result.y);
                let dir_length = length2(dx, dy);
                if dir_length < 0.1 {
                    i += 1;
                    continue;
                }
                let angle = direction_angle(dx, dy, dir_length);
                self.mcwp_info.camera_angle[i] = angle;
                i += 1;
            }
            if self.mcwp_info.total_time_milliseconds == 1 {
                // do it instantly.
                self.move_along_waypoint_path(view, 1, h);
                self.doing_move_camera_on_waypoint_path = true;
                self.camera_arrived_at_waypoint_on_path_flag = false;
            }
        }
    }

    /// `W3DView::cameraModFinalMoveTo(pLoc)` (W3DView.cpp:2661).
    pub fn camera_mod_final_move_to(&mut self, p_loc: &Coord3D) {
        if self.doing_rotate_camera {
            return; // Doesn't apply to rotate about a point.
        }
        if self.doing_move_camera_on_waypoint_path {
            let start = self.mcwp_info.waypoints[self.mcwp_info.num_waypoints as usize];
            let delta_x = p_loc.x - start.x;
            let delta_y = p_loc.y - start.y;
            for i in 2..=self.mcwp_info.num_waypoints as usize {
                let mut result = self.mcwp_info.waypoints[i];
                result.x += delta_x;
                result.y += delta_y;
                self.mcwp_info.waypoints[i] = result;
            }
        }
    }

    /// `W3DView::cameraModFinalLookToward(pLoc)` (W3DView.cpp:2684).
    pub fn camera_mod_final_look_toward(&mut self, p_loc: &Coord3D) {
        if self.doing_rotate_camera {
            return; // Doesn't apply to rotate about a point.
        }
        if self.doing_move_camera_on_waypoint_path {
            let mut min = self.mcwp_info.num_waypoints - 1;
            if min < 2 {
                min = 2;
            }
            let mut i = min;
            while i <= self.mcwp_info.num_waypoints {
                let iu = i as usize;
                let factor = 0.5f32;
                let start = midpoint_xy(&self.mcwp_info.waypoints[iu - 1], &self.mcwp_info.waypoints[iu]);
                let mid = self.mcwp_info.waypoints[iu];
                let end = midpoint_xy(&self.mcwp_info.waypoints[iu], &self.mcwp_info.waypoints[iu + 1]);
                let result = blend(&start, &mid, &end, factor);
                let (dx, dy) = (p_loc.x - result.x, p_loc.y - result.y);
                let dir_length = length2(dx, dy);
                if dir_length < 0.1 {
                    i += 1;
                    continue;
                }
                let mut angle = direction_angle(dx, dy, dir_length);
                if i == self.mcwp_info.num_waypoints {
                    self.mcwp_info.camera_angle[iu] = angle;
                } else {
                    let delta_angle = norm_angle(angle - self.mcwp_info.camera_angle[iu]);
                    angle = norm_angle(self.mcwp_info.camera_angle[iu] + delta_angle / 2.0);
                    self.mcwp_info.camera_angle[iu] = angle;
                }
                i += 1;
            }
        }
    }

    /// `W3DView::cameraModFinalTimeMultiplier(finalMultiplier)` (W3DView.cpp:2740). `REAL_TO_INT_FLOOR(0.5 + x)` is a
    /// `double` sum upstream (`0.5` is a `double` literal); it is taken in `f64` here.
    pub fn camera_mod_final_time_multiplier(&mut self, final_multiplier: i32) {
        if self.doing_zoom_camera {
            self.zc_info.end_time_multiplier = final_multiplier;
        }
        if self.doing_pitch_camera {
            self.pc_info.end_time_multiplier = final_multiplier;
        }
        if self.doing_rotate_camera {
            self.rc_info.end_time_multiplier = final_multiplier;
        } else if self.doing_move_camera_on_waypoint_path {
            let mut cur_distance = 0.0f32;
            for i in 0..self.mcwp_info.num_waypoints as usize {
                cur_distance += self.mcwp_info.way_seg_length[i];
                let factor2 = cur_distance / self.mcwp_info.total_distance;
                let factor1 = (1.0f64 - f64::from(factor2)) as f32;
                let v = 0.5f64 + f64::from(self.mcwp_info.time_multiplier[i + 1]) * f64::from(factor1) + f64::from(final_multiplier) * f64::from(factor2);
                self.mcwp_info.time_multiplier[i + 1] = v.floor() as i32;
            }
        } else {
            // If we aren't doing a camera movement, just set the time.
            self.time_multiplier = final_multiplier;
        }
    }

    /// `W3DView::cameraModRollingAverage(framesToAverage)` (W3DView.cpp:2766).
    pub fn camera_mod_rolling_average(&mut self, frames_to_average: i32) {
        let frames_to_average = if frames_to_average < 1 { 1 } else { frames_to_average };
        self.mcwp_info.rolling_average_frames = frames_to_average;
    }

    /// `W3DView::cameraModFinalPitch(finalPitch, easeIn, easeOut)` (W3DView.cpp:2775).
    pub fn camera_mod_final_pitch(&mut self, view: &TacticalView, final_pitch: f32, ease_in: f32, ease_out: f32) {
        if self.doing_rotate_camera {
            let time = ((self.rc_info.num_frames + self.rc_info.num_hold_frames - self.rc_info.cur_frame) * THE_W3D_FRAME_LENGTH_IN_MSEC) as f32;
            self.pitch_camera(view, final_pitch, time as i32, time * ease_in, time * ease_out);
        }
        if self.doing_move_camera_on_waypoint_path {
            let time = (self.mcwp_info.total_time_milliseconds - self.mcwp_info.elapsed_time_milliseconds) as f32;
            self.pitch_camera(view, final_pitch, time as i32, time * ease_in, time * ease_out);
        }
    }

    /// `W3DView::resetCamera(location, milliseconds, easeIn, easeOut)` (W3DView.cpp:2789): the general form of
    /// `TacticalView::reset_camera` (which gives only the instant end state).
    pub fn reset_camera(&mut self, view: &mut TacticalView, location: &Coord3D, milliseconds: i32, ease_in: f32, ease_out: f32, h: &impl ViewHost) {
        self.move_camera_to(view, location, milliseconds, 0, false, ease_in, ease_out, h);
        self.mcwp_info.camera_angle[2] = 0.0; // default angle.
        view.angle = self.mcwp_info.camera_angle[0];

        // terrain height + desired height offset == cameraOffset * actual zoom
        // find best approximation of max terrain height we can see
        let terrain_height_max = get_height_around_pos(h, location.x, location.y);
        let desired_height = terrain_height_max + view.max_height_above_ground;
        let desired_zoom = desired_height / view.camera_offset.z;

        self.zoom_camera(view, desired_zoom, milliseconds, ease_in, ease_out); // this isn't right... or is it?

        self.pitch_camera(view, 1.0, milliseconds, ease_in, ease_out);
    }

    /// `W3DView::isCameraMovementFinished()` (W3DView.cpp:2813). The motion-blur filter's in/out modes count as finished.
    pub fn is_camera_movement_finished(&self, blur: &MotionBlurEnums) -> bool {
        if self.view_filter == blur.motion_blur_filter && blur.in_out_modes.contains(&self.view_filter_mode) {
            // Several of the motion blur effects are similar to camera movements.
            return true;
        }
        !self.doing_move_camera_on_waypoint_path && !self.doing_rotate_camera && !self.doing_pitch_camera && !self.doing_zoom_camera
    }

    /// `W3DView::isCameraMovementAtWaypointAlongPath()` (W3DView.cpp:2830): the flag is cleared by the poll.
    pub fn is_camera_movement_at_waypoint_along_path(&mut self) -> bool {
        let return_value = self.camera_arrived_at_waypoint_on_path_flag;
        self.camera_arrived_at_waypoint_on_path_flag = false;
        return_value
    }

    /// `W3DView::moveCameraAlongWaypointPath(pWay, milliseconds, shutter, orient, easeIn, easeOut)` (W3DView.cpp:2844).
    /// `locations` is the waypoint chain from `pWay`, following `getLink(0)` each step (the caller walks the links:
    /// `pWay->getLocation()` of each, in order, ending where `getNumLinks() == 0`).
    pub fn move_camera_along_waypoint_path(&mut self, view: &mut TacticalView, locations: &[Coord3D], milliseconds: i32, shutter: i32, orient: bool, ease_in: f32, ease_out: f32, h: &impl ViewHost) {
        let min_delta = MAP_XY_FACTOR;

        self.mcwp_info.waypoints[0] = view.pos;
        self.mcwp_info.camera_angle[0] = view.angle;
        self.mcwp_info.way_seg_length[0] = 0.0;
        self.mcwp_info.waypoints[1] = view.pos;
        self.mcwp_info.num_waypoints = 1;
        let mut milliseconds = milliseconds;
        if milliseconds < 1 {
            milliseconds = 1;
        }
        self.mcwp_info.total_time_milliseconds = milliseconds;
        self.mcwp_info.shutter = shutter / THE_W3D_FRAME_LENGTH_IN_MSEC;
        if self.mcwp_info.shutter < 1 {
            self.mcwp_info.shutter = 1;
        }
        self.mcwp_info.ease.set_ease_times(ease_in / milliseconds as f32, ease_out / milliseconds as f32);

        let mut next = 0usize;
        // `while (pWay && numWaypoints < MAX_WAYPOINTS)`: pWay is non-null while `next < locations.len()`
        while next < locations.len() && (self.mcwp_info.num_waypoints as usize) < MAX_WAYPOINTS {
            self.mcwp_info.num_waypoints += 1;
            let n = self.mcwp_info.num_waypoints as usize;
            self.mcwp_info.waypoints[n] = locations[next];
            next += 1; // pWay = pWay->getLink(0), or NULL after the last
            let have_more = next < locations.len();
            let dir_x = self.mcwp_info.waypoints[n].x - self.mcwp_info.waypoints[n - 1].x;
            let dir_y = self.mcwp_info.waypoints[n].y - self.mcwp_info.waypoints[n - 1].y;
            if length2(dir_x, dir_y) < min_delta {
                if have_more {
                    self.mcwp_info.num_waypoints -= 1; // drop this one.
                } else {
                    self.mcwp_info.waypoints[n - 1] = self.mcwp_info.waypoints[n];
                    self.mcwp_info.num_waypoints -= 1; // Push this one back.
                }
            }
        }
        self.setup_waypoint_path(view, orient, h);
    }

    /// `W3DView::setupWaypointPath(orient)` (W3DView.cpp:2884).
    pub fn setup_waypoint_path(&mut self, view: &TacticalView, orient: bool, h: &impl ViewHost) {
        self.mcwp_info.cur_segment = 1;
        self.mcwp_info.cur_seg_distance = 0.0;
        self.mcwp_info.total_distance = 0.0;
        self.mcwp_info.rolling_average_frames = 1;
        let nw = self.mcwp_info.num_waypoints as usize;
        let mut angle = view.angle;
        for i in 1..nw {
            let dir_x = self.mcwp_info.waypoints[i + 1].x - self.mcwp_info.waypoints[i].x;
            let dir_y = self.mcwp_info.waypoints[i + 1].y - self.mcwp_info.waypoints[i].y;
            self.mcwp_info.way_seg_length[i] = length2(dir_x, dir_y);
            self.mcwp_info.total_distance += self.mcwp_info.way_seg_length[i];
            if orient {
                angle = direction_angle(dir_x, dir_y, self.mcwp_info.way_seg_length[i]);
            }
            self.mcwp_info.camera_angle[i] = angle;
        }
        self.mcwp_info.camera_angle[1] = view.angle;
        self.mcwp_info.camera_angle[nw] = self.mcwp_info.camera_angle[nw - 1];
        let mut i = nw - 1;
        while i > 1 {
            self.mcwp_info.camera_angle[i] = (self.mcwp_info.camera_angle[i] + self.mcwp_info.camera_angle[i - 1]) / 2.0;
            i -= 1;
        }
        self.mcwp_info.way_seg_length[nw + 1] = self.mcwp_info.way_seg_length[nw];

        // Prevent a possible divide by zero.
        if self.mcwp_info.total_distance < 1.0 {
            self.mcwp_info.way_seg_length[nw - 1] += 1.0 - self.mcwp_info.total_distance;
            self.mcwp_info.total_distance = 1.0;
        }

        let mut cur_distance = 0.0f32;
        let final_pos = self.mcwp_info.waypoints[nw];
        let new_ground = h.ground_height(final_pos.x, final_pos.y);
        for i in 0..=(nw + 1) {
            let factor2 = cur_distance / self.mcwp_info.total_distance;
            let factor1 = (1.0f64 - f64::from(factor2)) as f32;
            self.mcwp_info.time_multiplier[i] = self.time_multiplier;
            // (groundHeight has MAX_WAYPOINTS+1 entries; index nw+1 is written again below, as upstream)
            if i < self.mcwp_info.ground_height.len() {
                self.mcwp_info.ground_height[i] = view.ground_level * factor1 + new_ground * factor2;
            }
            cur_distance += self.mcwp_info.way_seg_length[i];
        }

        // Pad the end.
        self.mcwp_info.waypoints[nw + 1] = self.mcwp_info.waypoints[nw];
        let cur = self.mcwp_info.waypoints[nw];
        let prev = self.mcwp_info.waypoints[nw - 1];
        self.mcwp_info.waypoints[nw + 1].x += cur.x - prev.x;
        self.mcwp_info.waypoints[nw + 1].y += cur.y - prev.y;
        self.mcwp_info.camera_angle[nw + 1] = self.mcwp_info.camera_angle[nw];
        self.mcwp_info.ground_height[nw + 1] = new_ground;

        let cur = self.mcwp_info.waypoints[2];
        let prev = self.mcwp_info.waypoints[1];
        self.mcwp_info.waypoints[0].x -= cur.x - prev.x;
        self.mcwp_info.waypoints[0].y -= cur.y - prev.y;

        self.doing_move_camera_on_waypoint_path = self.mcwp_info.num_waypoints > 1;
        self.camera_arrived_at_waypoint_on_path_flag = false;
        self.doing_rotate_camera = false;

        self.mcwp_info.elapsed_time_milliseconds = 0;
        self.mcwp_info.cur_shutter = self.mcwp_info.shutter;
    }

    /// The `m_timeMultiplier` step of a rotation frame: `start + REAL_TO_INT_FLOOR(0.5 + (end-start)*factor)`.
    fn blended_time_multiplier(start: i32, end: i32, factor: f32) -> i32 {
        start + real_to_int_floor((0.5f64 + f64::from(end - start) * f64::from(factor)) as f32)
    }

    /// `W3DView::rotateCameraOneFrame()` (W3DView.cpp:2975).
    pub fn rotate_camera_one_frame(&mut self, view: &mut TacticalView, h: &impl ScriptedViewHost) {
        self.rc_info.cur_frame += 1;
        if h.global().disable_camera_movement {
            if self.rc_info.cur_frame >= self.rc_info.num_frames + self.rc_info.num_hold_frames {
                self.doing_rotate_camera = false;
                self.freeze_time_for_camera_movement = false;
            }
            return;
        }

        if self.rc_info.track_object {
            if self.rc_info.cur_frame <= self.rc_info.num_frames + self.rc_info.num_hold_frames {
                if let Some(p) = h.object_position(self.rc_info.target_object_id) {
                    // object has not been destroyed
                    self.rc_info.target_object_pos = p;
                }

                let dir_x = self.rc_info.target_object_pos.x - view.pos.x;
                let dir_y = self.rc_info.target_object_pos.y - view.pos.y;
                let dir_length = length2(dir_x, dir_y);
                if dir_length >= 0.1 {
                    let angle = direction_angle(dir_x, dir_y, dir_length);

                    if self.rc_info.cur_frame <= self.rc_info.num_frames {
                        let factor = self.rc_info.ease.eval(self.rc_info.cur_frame as f32 / self.rc_info.num_frames as f32);
                        let mut angle_diff = norm_angle(angle - view.angle);
                        angle_diff *= factor;
                        view.angle += angle_diff;
                        view.angle = norm_angle(view.angle);
                        self.time_multiplier = Self::blended_time_multiplier(self.rc_info.start_time_multiplier, self.rc_info.end_time_multiplier, factor);
                    } else {
                        view.angle = angle;
                    }
                }
            }
        } else if self.rc_info.cur_frame <= self.rc_info.num_frames {
            let factor = self.rc_info.ease.eval(self.rc_info.cur_frame as f32 / self.rc_info.num_frames as f32);
            view.angle = lerp(self.rc_info.start_angle, self.rc_info.end_angle, factor);
            view.angle = norm_angle(view.angle);
            self.time_multiplier = Self::blended_time_multiplier(self.rc_info.start_time_multiplier, self.rc_info.end_time_multiplier, factor);
        }

        if self.rc_info.cur_frame >= self.rc_info.num_frames + self.rc_info.num_hold_frames {
            self.doing_rotate_camera = false;
            self.freeze_time_for_camera_movement = false;
            if !self.rc_info.track_object {
                view.angle = self.rc_info.end_angle;
            }
        }
    }

    /// `W3DView::zoomCameraOneFrame()` (W3DView.cpp:3047).
    pub fn zoom_camera_one_frame(&mut self, view: &mut TacticalView, h: &impl ViewHost) {
        self.zc_info.cur_frame += 1;
        if h.global().disable_camera_movement {
            if self.zc_info.cur_frame >= self.zc_info.num_frames {
                self.doing_zoom_camera = false;
            }
            return;
        }
        if self.zc_info.cur_frame <= self.zc_info.num_frames {
            // not just holding; do the camera adjustment
            let factor = self.zc_info.ease.eval(self.zc_info.cur_frame as f32 / self.zc_info.num_frames as f32);
            view.zoom = lerp(self.zc_info.start_zoom, self.zc_info.end_zoom, factor);
        }

        if self.zc_info.cur_frame >= self.zc_info.num_frames {
            self.doing_zoom_camera = false;
            view.zoom = self.zc_info.end_zoom;
        }
    }

    /// `W3DView::pitchCameraOneFrame()` (W3DView.cpp:3073).
    pub fn pitch_camera_one_frame(&mut self, view: &mut TacticalView, h: &impl ViewHost) {
        self.pc_info.cur_frame += 1;
        if h.global().disable_camera_movement {
            if self.pc_info.cur_frame >= self.pc_info.num_frames {
                self.doing_pitch_camera = false;
            }
            return;
        }
        if self.pc_info.cur_frame <= self.pc_info.num_frames {
            // not just holding; do the camera adjustment
            let factor = self.pc_info.ease.eval(self.pc_info.cur_frame as f32 / self.pc_info.num_frames as f32);
            view.fx_pitch = lerp(self.pc_info.start_pitch, self.pc_info.end_pitch, factor);
        }

        if self.pc_info.cur_frame >= self.pc_info.num_frames {
            self.doing_pitch_camera = false;
            view.fx_pitch = self.pc_info.end_pitch;
        }
    }

    /// The camera offset from the global data's pitch and yaw that `moveAlongWaypointPath` re-derives (two places).
    fn rederive_camera_offset(view: &mut TacticalView, h: &impl ViewHost) {
        let g = h.global();
        view.camera_offset.y = (-(f64::from(view.camera_offset.z) / petir::Real::tan(f64::from(g.camera_pitch) * (f64::from(PI) / 180.0)))) as f32;
        view.camera_offset.x = (-(f64::from(view.camera_offset.y) * petir::Real::tan(f64::from(g.camera_yaw) * (f64::from(PI) / 180.0)))) as f32;
    }

    /// Widen the camera constraint to hold `p` ("assuming that the scripter knows what he is doing").
    fn widen_constraint(view: &mut TacticalView, p: &Coord3D) {
        view.camera_constraint.lo.x = view.camera_constraint.lo.x.min(p.x);
        view.camera_constraint.hi.x = view.camera_constraint.hi.x.max(p.x);
        view.camera_constraint.lo.y = view.camera_constraint.lo.y.min(p.y);
        view.camera_constraint.hi.y = view.camera_constraint.hi.y.max(p.y);
    }

    /// `W3DView::moveAlongWaypointPath(milliseconds)` (W3DView.cpp:3097).
    pub fn move_along_waypoint_path(&mut self, view: &mut TacticalView, milliseconds: i32, h: &impl ViewHost) {
        self.mcwp_info.elapsed_time_milliseconds += milliseconds;
        if h.global().disable_camera_movement {
            if self.mcwp_info.elapsed_time_milliseconds > self.mcwp_info.total_time_milliseconds {
                self.doing_move_camera_on_waypoint_path = false;
                self.freeze_time_for_camera_movement = false;
            }
            return;
        }
        let nw = self.mcwp_info.num_waypoints as usize;
        if self.mcwp_info.elapsed_time_milliseconds > self.mcwp_info.total_time_milliseconds {
            self.doing_move_camera_on_waypoint_path = false;
            self.camera_arrived_at_waypoint_on_path_flag = false;

            self.freeze_time_for_camera_movement = false;
            view.angle = self.mcwp_info.camera_angle[nw];

            view.ground_level = self.mcwp_info.ground_height[nw];
            Self::rederive_camera_offset(view, h);

            let mut pos = self.mcwp_info.waypoints[nw];
            pos.z = 0.0;
            view.pos = pos;
            Self::widen_constraint(view, &pos);
            return;
        }

        let total_time = self.mcwp_info.total_time_milliseconds as f32;
        let delta_time = self.mcwp_info.ease.eval(self.mcwp_info.elapsed_time_milliseconds as f32 / total_time)
            - self.mcwp_info.ease.eval((self.mcwp_info.elapsed_time_milliseconds - milliseconds) as f32 / total_time);
        self.mcwp_info.cur_seg_distance += delta_time * self.mcwp_info.total_distance;
        while self.mcwp_info.cur_seg_distance >= self.mcwp_info.way_seg_length[self.mcwp_info.cur_segment as usize] {
            if self.doing_move_camera_on_waypoint_path {
                self.camera_arrived_at_waypoint_on_path_flag = true;
            }

            self.mcwp_info.cur_seg_distance -= self.mcwp_info.way_seg_length[self.mcwp_info.cur_segment as usize];
            self.mcwp_info.cur_segment += 1;
            if self.mcwp_info.cur_segment >= self.mcwp_info.num_waypoints {
                self.mcwp_info.total_time_milliseconds = 0; // Will end following next frame.
                return;
            }
        }
        let mut avg_factor = (1.0f64 / f64::from(self.mcwp_info.rolling_average_frames)) as f32;
        self.mcwp_info.cur_shutter -= 1;
        if self.mcwp_info.cur_shutter > 0 {
            return;
        }
        self.mcwp_info.cur_shutter = self.mcwp_info.shutter;
        let seg = self.mcwp_info.cur_segment as usize;
        let mut factor = self.mcwp_info.cur_seg_distance / self.mcwp_info.way_seg_length[seg];
        if self.mcwp_info.cur_segment == self.mcwp_info.num_waypoints - 1 {
            avg_factor = avg_factor + (1.0 - avg_factor) * factor;
        }
        let factor1 = (1.0f64 - f64::from(factor)) as f32;
        let factor2 = (1.0f64 - f64::from(factor1)) as f32;
        let mut angle1 = self.mcwp_info.camera_angle[seg];
        let angle2 = self.mcwp_info.camera_angle[seg + 1];
        if angle2 - angle1 > PI {
            angle1 += 2.0 * PI;
        }
        if angle2 - angle1 < -PI {
            angle1 -= 2.0 * PI;
        }
        let mut angle = angle1 * factor1 + angle2 * factor2;

        angle = norm_angle(angle);
        let delta_angle = norm_angle(angle - view.angle);
        // (upstream: `if (fabs(deltaAngle) > PI/10) DEBUG_LOG(("Huh.\n"));` — log only)
        view.angle += avg_factor * delta_angle;
        view.angle = norm_angle(view.angle);

        let time_multiplier = self.mcwp_info.time_multiplier[seg] as f32 * factor1 + self.mcwp_info.time_multiplier[seg + 1] as f32 * factor2;
        self.time_multiplier = real_to_int_floor((0.5f64 + f64::from(time_multiplier)) as f32);

        view.ground_level = self.mcwp_info.ground_height[seg] * factor1 + self.mcwp_info.ground_height[seg + 1] * factor2;
        Self::rederive_camera_offset(view, h);

        let (start, mid, end);
        if factor < 0.5 {
            start = midpoint_xy(&self.mcwp_info.waypoints[seg - 1], &self.mcwp_info.waypoints[seg]);
            mid = self.mcwp_info.waypoints[seg];
            end = midpoint_xy(&self.mcwp_info.waypoints[seg], &self.mcwp_info.waypoints[seg + 1]);
            factor += 0.5;
        } else {
            start = midpoint_xy(&self.mcwp_info.waypoints[seg], &self.mcwp_info.waypoints[seg + 1]);
            mid = self.mcwp_info.waypoints[seg + 1];
            end = midpoint_xy(&self.mcwp_info.waypoints[seg + 1], &self.mcwp_info.waypoints[seg + 2]);
            factor -= 0.5;
        }

        let result = blend(&start, &mid, &end, factor);
        view.pos = result;
        Self::widen_constraint(view, &result);
    }

    /// `W3DView::updateCameraMovements()` (W3DView.cpp:1039): one frame of every scripted movement; `true` if any ran.
    pub fn update_camera_movements(&mut self, view: &mut TacticalView, h: &impl ScriptedViewHost) -> bool {
        let mut did_update = false;

        if self.doing_zoom_camera {
            self.zoom_camera_one_frame(view, h);
            did_update = true;
        }
        if self.doing_pitch_camera {
            self.pitch_camera_one_frame(view, h);
            did_update = true;
        }
        if self.doing_rotate_camera {
            self.previous_look_at_position = view.pos;
            self.rotate_camera_one_frame(view, h);
            did_update = true;
        } else if self.doing_move_camera_on_waypoint_path {
            self.previous_look_at_position = view.pos;
            self.move_along_waypoint_path(view, THE_W3D_FRAME_LENGTH_IN_MSEC, h);
            did_update = true;
        }
        if self.doing_scripted_camera_lock {
            did_update = true;
        }
        did_update
    }

    /// `W3DView::update()` (W3DView.cpp:1084) up to the camera transform: the camera lock's follow, the scripted
    /// movements, the shake and the height above ground (which `TacticalView::update_height` gives for the
    /// no-scripted-movement case; this is that block with `didScriptedMovement`). The drawable iteration at its end is
    /// the renderer's. `TheTerrainRenderObject->doesNeedFullUpdate()`/`updateCenter` at its start are device-only.
    pub fn update(&mut self, view: &mut TacticalView, h: &mut impl ScriptedViewHost) {
        let mut recalc_camera = false;
        let mut did_scripted_movement = false;

        let camera_lock = self.camera_lock;
        if camera_lock == INVALID_ID {
            self.follow_factor = -1.0;
        }
        if camera_lock != INVALID_ID {
            self.doing_move_camera_on_waypoint_path = false;
            self.camera_arrived_at_waypoint_on_path_flag = false;

            let camera_lock_obj = h.locked_object(camera_lock);
            // check if object has been destroyed or is dead -> lose lock
            let lose_lock = camera_lock_obj.is_none();

            if lose_lock {
                self.camera_lock = INVALID_ID; // setCameraLock(INVALID_ID)
                self.lock_dist = 0.0;
                self.lock_type = CameraLockType::Follow;
                self.doing_scripted_camera_lock = false;
                // setCameraLockDrawable(NULL): the drawable lock is the host's
                self.follow_factor = -1.0;
            } else if let Some(obj) = camera_lock_obj {
                if self.follow_factor < 0.0 {
                    self.follow_factor = 0.05;
                } else {
                    self.follow_factor += 0.05;
                    if self.follow_factor > 1.0 {
                        self.follow_factor = 1.0;
                    }
                }
                if h.locked_drawable_camera() {
                    // the camera-lock drawable placed the camera itself: `recalcCamera = false`
                    recalc_camera = false;
                } else {
                    let objpos = obj.position;
                    let mut curpos = view.pos;
                    // don't "snap" directly to the pos, but move there smoothly.
                    let g = h.global();
                    let snap_thresh_sqr = g.partition_cell_size * g.partition_cell_size;
                    let cur_dist_sqr = (curpos.x - objpos.x) * (curpos.x - objpos.x) + (curpos.y - objpos.y) * (curpos.y - objpos.y);
                    if self.snap_immediate {
                        // close enough.
                        curpos.x = objpos.x;
                        curpos.y = objpos.y;
                    } else {
                        let dx = objpos.x - curpos.x;
                        let dy = objpos.y - curpos.y;
                        if self.lock_type == CameraLockType::Tether {
                            if cur_dist_sqr >= snap_thresh_sqr {
                                let ratio = 1.0 - snap_thresh_sqr / cur_dist_sqr;

                                // move halfway there.
                                curpos.x += dx * ratio * 0.5;
                                curpos.y += dy * ratio * 0.5;
                            } else {
                                // we're inside our 'play' tolerance.  Move slowly to the obj
                                let ratio = 0.01 * self.lock_dist;
                                let dx = objpos.x - curpos.x;
                                let dy = objpos.y - curpos.y;
                                curpos.x += dx * ratio;
                                curpos.y += dy * ratio;
                            }
                        } else {
                            curpos.x += dx * self.follow_factor;
                            curpos.y += dy * self.follow_factor;
                        }
                    }
                    if !(h.time_frozen_debug() || h.time_frozen_script()) && !h.game_paused() {
                        self.previous_look_at_position = view.pos;
                    }
                    view.pos = curpos;

                    if self.lock_type == CameraLockType::Follow {
                        // camera follow objects if they are flying
                        if obj.airborne_above_terrain {
                            let old_z_rot = norm_angle(view.angle);
                            let ideal_z_rot = norm_angle(obj.orientation - std::f32::consts::FRAC_PI_2);
                            let diff = norm_angle(ideal_z_rot - old_z_rot);

                            if self.snap_immediate {
                                view.angle = ideal_z_rot;
                            } else {
                                view.angle += diff * 0.1;
                            }
                            view.angle = norm_angle(view.angle);
                        }
                    }
                    if self.snap_immediate {
                        self.snap_immediate = false;
                    }

                    view.ground_level = objpos.z;
                    did_scripted_movement = true;
                    recalc_camera = true;
                }
            }
        }

        if !h.time_frozen_debug() && !h.game_paused() {
            // If we aren't frozen for debug, allow the camera to follow scripted movements.
            if self.update_camera_movements(view, h) {
                did_scripted_movement = true;
                recalc_camera = true;
            }
        } else if self.doing_rotate_camera || self.doing_move_camera_on_waypoint_path || self.doing_pitch_camera || self.doing_zoom_camera || self.doing_scripted_camera_lock {
            did_scripted_movement = true; // don't mess up the scripted movement
        }

        // Process camera shake (@todo upstream: make this framerate-independent)
        if self.shake_intensity > 0.01 {
            self.shake_offset.x = self.shake_intensity * self.shake_angle_cos;
            self.shake_offset.y = self.shake_intensity * self.shake_angle_sin;

            // fake a stiff spring/damper
            let damping_coeff = 0.75f32;
            self.shake_intensity *= damping_coeff;

            // spring is so "stiff", it pulls 180 degrees opposite each frame
            self.shake_angle_cos = -self.shake_angle_cos;
            self.shake_angle_sin = -self.shake_angle_sin;

            recalc_camera = true;
        } else {
            self.shake_intensity = 0.0;
            self.shake_offset.x = 0.0;
            self.shake_offset.y = 0.0;
        }

        // Process New C3 Camera Shaker system
        if self.camera_shaker_system.is_camera_shaking() {
            recalc_camera = true;
        }

        // the height above ground (W3DView.cpp:1330)
        view.terrain_height_under_camera = get_height_around_pos(h, view.pos.x, view.pos.y);
        view.current_height_above_ground = view.camera_offset.z * view.zoom - view.terrain_height_under_camera;
        if view.ok_to_adjust_height && !h.game_paused() {
            let desired_height = view.terrain_height_under_camera + view.height_above_ground;
            let desired_zoom = desired_height / view.camera_offset.z;
            let use_camera_in_replay = h.global().use_camera_in_replay;
            if did_scripted_movement || (h.in_replay_game() && use_camera_in_replay) {
                // if we are in a scripted camera movement, take its height above ground as our desired height.
                view.height_above_ground = view.current_height_above_ground;
            }
            let g = h.global();
            if h.ui_scrolling() {
                // if scrolling, only adjust if we're too close or too far
                if view.scroll_amount.length() < view.scroll_amount_cutoff
                    || (view.current_height_above_ground < view.min_height_above_ground)
                    || (g.enforce_max_camera_height && view.current_height_above_ground > view.max_height_above_ground)
                {
                    let zoom_adj = (desired_zoom - view.zoom) * g.camera_adjust_speed;
                    if f64::from(rts_lockstep_engine::ww::wwmath::fabs(zoom_adj)) >= 0.0001 {
                        view.zoom += zoom_adj;
                        recalc_camera = true;
                    }
                }
            } else {
                // we're not scrolling; settle toward desired height above ground
                let zoom_adj = (view.zoom - desired_zoom) * g.camera_adjust_speed;
                let zoom_adj_abs = rts_lockstep_engine::ww::wwmath::fabs(zoom_adj);
                if f64::from(zoom_adj_abs) >= 0.0001 && !did_scripted_movement {
                    view.zoom -= zoom_adj;
                    recalc_camera = true;
                }
            }
        }
        if h.time_fast() {
            return; // don't draw - makes it faster :) jba.
        }

        // (gth) C&C3 if m_isCameraSlaved then force the camera to update each frame
        if recalc_camera || self.is_camera_slaved {
            view.set_camera_transform(h);
        }
    }

    /// `W3DView::shake(epicenter, shakeType)` (W3DView.cpp:3248). Upstream clamps with a quirk kept here: when the sum
    /// passes `m_maxShakeIntensity` it is set to the local `maxIntensity` (3.0), not to the global.
    pub fn shake(&mut self, view: &TacticalView, epicenter: &Coord3D, shake_type: CameraShakeType, h: &mut impl ScriptedViewHost) {
        let angle = h.game_client_random_value_real(0.0, 2.0 * PI);

        self.shake_angle_cos = petir::Real::cos(f64::from(angle)) as f32;
        self.shake_angle_sin = petir::Real::sin(f64::from(angle)) as f32;

        let g = h.global();
        let intensity = match shake_type {
            CameraShakeType::Subtle => g.shake_subtle_intensity,
            CameraShakeType::Normal => g.shake_normal_intensity,
            CameraShakeType::Strong => g.shake_strong_intensity,
            CameraShakeType::Severe => g.shake_severe_intensity,
            CameraShakeType::CineExtreme => g.shake_cine_extreme_intensity,
            CameraShakeType::CineInsane => g.shake_cine_insane_intensity,
        };

        // intensity falls off with distance
        let view_pos = view.pos;
        let dx = epicenter.x - view_pos.x;
        let dy = epicenter.y - view_pos.y;
        // @todo upstream: make this 3D once we have the real "lookat" spot

        let dist = petir::Real::sqrt(f64::from(dx * dx + dy * dy)) as f32;

        if dist > g.max_shake_range {
            return;
        }

        let mut intensity = intensity;
        intensity *= 1.0 - (dist / g.max_shake_range);

        // add intensity and clamp
        self.shake_intensity += intensity;

        let max_intensity = 3.0f32;
        if self.shake_intensity > g.max_shake_intensity {
            self.shake_intensity = max_intensity;
        }
    }

    /// `W3DView::Add_Camera_Shake(position, radius, duration, power)` (W3DView.cpp:3352): `CameraShakerSystem.Add_Camera_Shake`.
    pub fn add_camera_shake(&mut self, position: &Coord3D, radius: f32, duration: f32, power: f32, h: &mut impl ScriptedViewHost) {
        let vpos = Vector3::new(position.x, position.y, position.z);
        let mut unit = || h.unit_random();
        self.camera_shaker_system.add_camera_shake(&vpos, radius, duration, power, &mut unit);
    }

    /// `W3DView::cameraEnableSlaveMode(objectName, boneName)` (W3DView.cpp:3322).
    pub fn camera_enable_slave_mode(&mut self, object_name: &[u8], bone_name: &[u8]) {
        self.is_camera_slaved = true;
        self.camera_slave_object_name = object_name.to_vec();
        self.camera_slave_object_bone_name = bone_name.to_vec();
    }

    /// `W3DView::cameraDisableSlaveMode()` (W3DView.cpp:3329).
    pub fn camera_disable_slave_mode(&mut self) {
        self.is_camera_slaved = false;
    }

    /// `W3DView::cameraEnableRealZoomMode()` (W3DView.cpp:3334): `updateView()` is `update()`, the caller's next call.
    pub fn camera_enable_real_zoom_mode(&mut self, view: &mut TacticalView) {
        self.use_real_zoom_cam = true;
        view.fx_pitch = 1.0; // Reset to default
    }

    /// `W3DView::cameraDisableRealZoomMode()` (W3DView.cpp:3342): `m_FOV = 50 degrees`, the transform, then `updateView()`.
    pub fn camera_disable_real_zoom_mode(&mut self, view: &mut TacticalView, h: &impl ViewHost) {
        self.use_real_zoom_cam = false;
        view.fx_pitch = 1.0; // Reset to default
        view.fov = 50.0 * PI / 180.0;
        view.set_camera_transform(h);
    }

    /// `W3DView::setViewFilterMode(filterMode)` (W3DView.cpp:1465), with `W3DShaderManager::filterSetup` always succeeding
    /// (device-only, not translated).
    pub fn set_view_filter_mode(&mut self, filter_mode: i32) -> bool {
        self.view_filter_mode = filter_mode;
        true
    }

    /// `W3DView::setViewFilter(filter)` (W3DView.cpp:1483), with `W3DShaderManager::filterSetup` always succeeding.
    pub fn set_view_filter(&mut self, filter: i32) -> bool {
        self.view_filter = filter;
        true
    }
}

impl Default for ScriptedCamera {
    fn default() -> Self {
        ScriptedCamera::new()
    }
}

/// `PATHFIND_CELL_SIZE_F` is `W3DView::lookAt`'s height test, in `TacticalView::look_at` (not duplicated); kept referenced.
#[allow(dead_code)]
const _LOOK_AT_HEIGHT_TEST: f32 = PATHFIND_CELL_SIZE_F;
