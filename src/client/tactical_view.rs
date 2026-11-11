// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/View.cpp (View::View, init, zoomIn, zoomOut, setAngle, setPitch,
//     setAngleAndPitchToDefault, getLocation, setLocation)
//   GeneralsMD/Code/GameEngine/Include/GameClient/View.h (ViewLocation, the inline accessors)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DView.cpp (normAngle, getHeightAroundPos,
//     W3DView::W3DView, setHeight, setWidth, buildCameraTransform, calcCameraConstraints, getPickRay,
//     setCameraTransform, init, update's height above ground, scrollBy, forceRedraw, setAngle, setPitch,
//     setAngleAndPitchToDefault, setDefaultView, setHeightAboveGround, setZoom, setZoomToDefault, lookAt,
//     initHeightForMap; #131: resetCamera with the end of the 1 ms moveCameraTo, zoomCamera and pitchCamera it starts:
//     setupWaypointPath, moveAlongWaypointPath, zoomCameraOneFrame, pitchCameraOneFrame, updateCameraMovements)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/camera.cpp (CameraClass: the constructor, Set_View_Plane,
//     Set_Aspect_Ratio, Un_Project, Device_To_View_Space, Device_To_World_Space)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/Common/W3DConvert.cpp (PixelScreenToW3DLogicalScreen)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `TheTacticalView` (GitHub #119): the pieces of `View` and `W3DView` that `LookAtTranslator` calls, and what they
//! need to place the camera. The client's Bevy camera is put where [`TacticalView::camera_transform`] says.
//!
//! # As upstream reads it
//!
//! The view looks at a ground point (`m_pos`) from the camera offset (`m_cameraOffset`, from the global data's camera
//! pitch, yaw and height), scaled by the zoom, pitched, turned by the angle, and pulled in to the ground level
//! (`buildCameraTransform`). The player sets a desired height above the ground (`zoomIn` and `zoomOut` move it by 10,
//! clamped to the minimum and maximum camera heights); each frame the zoom settles a tenth of the way toward it
//! ([`TacticalView::update_height`], `W3DView::update`). `scrollBy` moves the look-at point by the world distance a
//! screen delta of 250 pixels per unit spans on the camera's view plane. `setCameraTransform` keeps the look-at point
//! inside the map, by the distance from the screen's centre to 95 % down it (`calcCameraConstraints`).
//!
//! # Upstream quirks that are kept
//!
//! - `normAngle` sets an angle beyond ±10π to 0 before folding it into ±π.
//! - `setZoomToDefault` sets the zoom with no clamp to the zoom limits (`setZoom` clamps).
//! - `setDefaultView` ignores its angle (commented out upstream: "we no longer want to rotate maps").
//! - `scrollBy`'s aspect is `getWidth()/getHeight()`, an integer division: 1 on a 4:3 display, 0 on one taller than
//!   wide (see [`TacticalView::portrait_scroll`], the one addition).
//! - `buildCameraTransform`'s pull-in factor is `1.0 - groundLevel/z` in `double`, rounded to `float`.
//!
//! # Where this differs from upstream
//!
//! - Not translated (not reached by `LookAtTranslator` in a release build, nor by the client): the scripted camera
//!   (`moveCameraTo`, `rotateCamera`, the waypoint paths, `cameraMod*`, `updateCameraMovements` and the `m_doing*`
//!   flags they clear; **#131:** but for `resetCamera`'s instant form, whose end state [`TacticalView::reset_camera`]
//!   gives), the camera lock and its follow in `update`, the camera shake (`m_shakeOffset` and
//!   `CameraShakerSystem`: their angles are 0, a rotation by nothing), the slaved and real-zoom cameras
//!   (`m_isCameraSlaved`, `m_useRealZoomCam`, both false), the field of view (debug only), picking and drawing.
//! - `lookAt` takes the on-ground branch only: a point higher than a pathfind cell above the ground would be cast
//!   along the view ray onto the terrain render object, which the port does not have (GitHub #119's gaps).
//! - `TheTerrainLogic`, the global data and the game's pause are given through [`ViewHost`]; the display's size is the
//!   view's (the client's tactical view is the whole window; upstream's is 77 % of its height, the control bar below).
//!
//! # Representation
//!
//! [`TacticalView`] holds `View`'s and `W3DView`'s members that these functions read, and [`ViewCamera`] the
//! `CameraClass` members (`Transform`, `ViewPlane`, `Viewport`, `AspectRatio`, the clip planes). `ViewLocation` is
//! [`ViewLocation`] with public fields.

use crate::common::base_type::{max, Coord2D, Coord3D, ICoord2D, Region2D, Region3D, PI};
use crate::common::global_data::GlobalData;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath::WWMATH_PI;

/// `DEFAULT_VIEW_WIDTH`.
pub const DEFAULT_VIEW_WIDTH: i32 = 640;
/// `DEFAULT_VIEW_HEIGHT`.
pub const DEFAULT_VIEW_HEIGHT: i32 = 480;
/// `MAP_XY_FACTOR`.
const MAP_XY_FACTOR: f32 = 10.0;
/// `TERRAIN_SAMPLE_SIZE`.
const TERRAIN_SAMPLE_SIZE: f32 = 40.0;
/// `SCROLL_RESOLUTION` (`W3DView::scrollBy`).
const SCROLL_RESOLUTION: f32 = 250.0;
/// `PATHFIND_CELL_SIZE_F` (`lookAt`'s height test).
const PATHFIND_CELL_SIZE_F: f32 = 10.0;

/// What the view asks of the engine.
pub trait ViewHost {
    /// `TheGlobalData`.
    fn global(&self) -> &GlobalData;
    /// `TheTerrainLogic->getGroundHeight(x, y)`.
    fn ground_height(&self, x: f32, y: f32) -> f32;
    /// `TheTerrainLogic->getExtent(&region)`; `None` without a terrain (`if (TheTerrainLogic)`).
    fn extent(&self) -> Option<Region3D>;
    /// `TheGameLogic->isGamePaused()`.
    fn game_paused(&self) -> bool;
}

/// `ViewLocation`: a saved view (the bookmarks).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewLocation {
    /// `m_valid`.
    pub valid: bool,
    /// `m_pos`.
    pub pos: Coord3D,
    /// `m_angle`.
    pub angle: f32,
    /// `m_pitch`.
    pub pitch: f32,
    /// `m_zoom`.
    pub zoom: f32,
}

impl ViewLocation {
    /// `init(x, y, z, angle, pitch, zoom)`: sets the fields and `m_valid`.
    pub fn init(&mut self, x: f32, y: f32, z: f32, angle: f32, pitch: f32, zoom: f32) {
        self.pos = Coord3D { x, y, z };
        self.angle = angle;
        self.pitch = pitch;
        self.zoom = zoom;
        self.valid = true;
    }
}

/// `CameraClass`'s members that the view reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewCamera {
    /// `Transform` (`Set_Transform`, `Get_Transform`).
    pub transform: Matrix3D,
    /// `ViewPlane.Min`, `ViewPlane.Max`.
    pub view_plane: (Vector2, Vector2),
    /// `Viewport.Min`, `Viewport.Max` (fractions of the render target).
    pub viewport: (Vector2, Vector2),
    /// `AspectRatio`.
    pub aspect_ratio: f32,
    /// `ZNear`, `ZFar`.
    pub clip: (f32, f32),
}

impl ViewCamera {
    /// `CameraClass()`: the identity transform, the whole target, 4:3, clip 1 .. 1000, a 50 degree view plane.
    pub fn new() -> Self {
        let mut c = ViewCamera {
            transform: Matrix3D::identity(),
            view_plane: (Vector2::new(-1.0, -1.0), Vector2::new(1.0, 1.0)),
            viewport: (Vector2::new(0.0, 0.0), Vector2::new(1.0, 1.0)),
            aspect_ratio: 4.0f32 / 3.0f32,
            clip: (1.0, 1000.0),
        };
        c.set_view_plane_fov(deg_to_radf(50.0), -1.0);
        c
    }

    /// `Set_View_Plane(hfov, vfov)`: `tan` in `double`; a `vfov` of -1 keeps the aspect ratio.
    pub fn set_view_plane_fov(&mut self, hfov: f32, vfov: f32) {
        let width_half = petir::Real::tan(f64::from(hfov) / 2.0) as f32;
        let height_half = if vfov == -1.0 {
            (1.0f32 / self.aspect_ratio) * width_half
        } else {
            let h = petir::Real::tan(f64::from(vfov) / 2.0) as f32;
            self.aspect_ratio = width_half / h;
            h
        };
        self.view_plane = (Vector2::new(-width_half, -height_half), Vector2::new(width_half, height_half));
    }

    /// `Set_Aspect_Ratio(width_to_height)`: the view plane's height follows its width.
    pub fn set_aspect_ratio(&mut self, width_to_height: f32) {
        self.aspect_ratio = width_to_height;
        self.view_plane.0.y = self.view_plane.0.x / self.aspect_ratio;
        self.view_plane.1.y = self.view_plane.1.x / self.aspect_ratio;
    }

    /// `Get_Position()`: the transform's translation.
    pub fn position(&self) -> Vector3 {
        self.transform.get_translation()
    }

    /// `Get_Depth()`: the far clip plane's distance.
    pub fn depth(&self) -> f32 {
        self.clip.1
    }

    /// `Un_Project(dest, view_point)`: a point of the view plane (`view_point` in -1 .. 1) in world space.
    pub fn un_project(&self, view_point: Vector2) -> Vector3 {
        let (min, max) = self.view_plane;
        let vpdx = max.x - min.x;
        let vpdy = max.y - min.y;
        let point = Vector3::new(min.x + vpdx * (view_point.x + 1.0f32) * 0.5f32, min.y + vpdy * (view_point.y + 1.0f32) * 0.5f32, -1.0f32);
        Matrix3D::transform_vector(&self.transform, &point)
    }

    /// `Device_To_World_Space(device, &world)` (through `Device_To_View_Space`): a pixel of the render target
    /// (`res_width` by `res_height`) on the view plane at z = -1, in world space.
    pub fn device_to_world_space(&self, device: Vector2, res_width: i32, res_height: i32) -> Vector3 {
        let ndev = Vector2::new(device.x / res_width as f32, device.y / res_height as f32);
        let (vs_min, vs_max) = self.view_plane;
        let (vp_min, vp_max) = self.viewport;
        let view = Vector3::new(
            vs_min.x + (ndev.x - vp_min.x) * (vs_max.x - vs_min.x) / (vp_max.x - vp_min.x),
            vs_max.y - (ndev.y - vp_min.y) * (vs_max.y - vs_min.y) / (vp_max.y - vp_min.y),
            -1.0f32,
        );
        Matrix3D::transform_vector(&self.transform, &view)
    }
}

impl Default for ViewCamera {
    fn default() -> Self {
        Self::new()
    }
}

/// `DEG_TO_RADF(x)`: `x * WWMATH_PI / 180.0f`.
fn deg_to_radf(x: f32) -> f32 {
    x * WWMATH_PI / 180.0f32
}

/// `minf(a, b)`.
fn minf(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

/// `maxf(a, b)`.
fn maxf(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

/// `normAngle(angle)`: an angle past ±10π is 0, then folded into ±π.
pub fn norm_angle(mut angle: f32) -> f32 {
    if angle < -10.0f32 * PI {
        angle = 0.0;
    }
    if angle > 10.0f32 * PI {
        angle = 0.0;
    }
    while angle < -PI {
        angle += 2.0f32 * PI;
    }
    while angle > PI {
        angle -= 2.0f32 * PI;
    }
    angle
}

/// `getHeightAroundPos(x, y)`: the highest ground at the point and 40 units diagonally around it.
pub fn get_height_around_pos(h: &impl ViewHost, x: f32, y: f32) -> f32 {
    let terrain_height = h.ground_height(x, y);
    let mut m = terrain_height;
    m = max(m, h.ground_height(x + TERRAIN_SAMPLE_SIZE, y - TERRAIN_SAMPLE_SIZE));
    m = max(m, h.ground_height(x - TERRAIN_SAMPLE_SIZE, y - TERRAIN_SAMPLE_SIZE));
    m = max(m, h.ground_height(x + TERRAIN_SAMPLE_SIZE, y + TERRAIN_SAMPLE_SIZE));
    m = max(m, h.ground_height(x - TERRAIN_SAMPLE_SIZE, y + TERRAIN_SAMPLE_SIZE));
    m
}

/// `PixelScreenToW3DLogicalScreen(x, y, &lx, &ly, w, h)`: a pixel (0,0 top left) to -1 .. 1 (y up).
pub fn pixel_screen_to_w3d_logical_screen(screen_x: i32, screen_y: i32, screen_width: i32, screen_height: i32) -> (f32, f32) {
    let log_x = ((2.0f32 * screen_x as f32) / screen_width as f32) - 1.0f32;
    let log_y = -(((2.0f32 * screen_y as f32) / screen_height as f32) - 1.0f32);
    (log_x, log_y)
}

/// `W3DView` (with `View`'s members): the tactical view's camera.
#[derive(Clone, Debug, PartialEq)]
pub struct TacticalView {
    /// `m_pos`: the point looked at.
    pub pos: Coord3D,
    /// `m_width`, `m_height`.
    pub width: i32,
    pub height: i32,
    /// `m_originX`, `m_originY`.
    pub origin: ICoord2D,
    /// `m_angle`.
    pub angle: f32,
    /// `m_pitchAngle`.
    pub pitch_angle: f32,
    /// `m_zoom`, `m_maxZoom`, `m_minZoom`.
    pub zoom: f32,
    pub max_zoom: f32,
    pub min_zoom: f32,
    /// `m_heightAboveGround` (the height the player asked for), `m_maxHeightAboveGround`, `m_minHeightAboveGround`.
    pub height_above_ground: f32,
    pub max_height_above_ground: f32,
    pub min_height_above_ground: f32,
    /// `m_terrainHeightUnderCamera`, `m_currentHeightAboveGround`.
    pub terrain_height_under_camera: f32,
    pub current_height_above_ground: f32,
    /// `m_defaultAngle`, `m_defaultPitchAngle`.
    pub default_angle: f32,
    pub default_pitch_angle: f32,
    /// `m_FOV`.
    pub fov: f32,
    /// `m_mouseLocked`.
    pub mouse_locked: bool,
    /// `m_zoomLimited`.
    pub zoom_limited: bool,
    /// `m_okToAdjustHeight`.
    pub ok_to_adjust_height: bool,
    /// `W3DView::m_3DCamera`.
    pub camera: ViewCamera,
    /// `m_groundLevel`.
    pub ground_level: f32,
    /// `m_cameraOffset`.
    pub camera_offset: Coord3D,
    /// `m_FXPitch`.
    pub fx_pitch: f32,
    /// `m_scrollAmount` (`scrollBy`'s last delta) and `m_scrollAmountCutoff`.
    pub scroll_amount: Coord2D,
    pub scroll_amount_cutoff: f32,
    /// `m_cameraConstraint`, `m_cameraConstraintValid`.
    pub camera_constraint: Region2D,
    pub camera_constraint_valid: bool,
    /// `m_cameraHasMovedSinceRequest`.
    pub camera_has_moved_since_request: bool,
    /// **New (not upstream), GitHub #119:** `scrollBy`'s aspect is an integer division, 0 on a display taller than
    /// wide (a phone held upright), which stops the vertical scroll. Set by the client on such a display: the aspect is
    /// then 1 (what upstream's own 4:3 displays get). False by default, as upstream.
    pub portrait_scroll: bool,
}

impl TacticalView {
    /// `View::View()` then `W3DView::W3DView()`: the camera offset from the global data's camera height, pitch and yaw
    /// (degrees; `tan` in `double`).
    pub fn new(g: &GlobalData) -> Self {
        let z = g.camera_height;
        let y = (-(f64::from(z) / petir::Real::tan(f64::from(g.camera_pitch) * (f64::from(PI) / 180.0)))) as f32;
        let x = (-(f64::from(y) * petir::Real::tan(f64::from(g.camera_yaw) * (f64::from(PI) / 180.0)))) as f32;
        TacticalView {
            pos: Coord3D::default(),
            width: 0,
            height: 0,
            origin: ICoord2D::default(),
            angle: 0.0,
            pitch_angle: 0.0,
            zoom: 0.0,
            max_zoom: 0.0,
            min_zoom: 0.0,
            height_above_ground: 0.0,
            max_height_above_ground: 0.0,
            min_height_above_ground: 0.0,
            terrain_height_under_camera: 0.0,
            current_height_above_ground: 0.0,
            default_angle: 0.0,
            default_pitch_angle: 0.0,
            fov: 50.0f32 * PI / 180.0f32,
            mouse_locked: false,
            zoom_limited: true,
            ok_to_adjust_height: true,
            camera: ViewCamera::new(),
            ground_level: 10.0,
            camera_offset: Coord3D { x, y, z },
            fx_pitch: 1.0,
            scroll_amount: Coord2D::default(),
            scroll_amount_cutoff: 0.0,
            camera_constraint: Region2D::default(),
            camera_constraint_valid: false,
            camera_has_moved_since_request: true,
            portrait_scroll: false,
        }
    }

    /// `W3DView::init()` (with `View::init()`): the default size and zoom limits, the global data's camera heights,
    /// the look-at point at (870, 770), and the camera placed.
    pub fn init(&mut self, h: &impl ViewHost) {
        let g = h.global();
        self.width = DEFAULT_VIEW_WIDTH;
        self.height = DEFAULT_VIEW_HEIGHT;
        self.origin = ICoord2D::default();
        self.pos.x = 0.0;
        self.pos.y = 0.0;
        self.angle = 0.0;
        self.zoom_limited = true;
        self.max_zoom = 1.3;
        self.min_zoom = 0.2;
        self.zoom = self.max_zoom;
        self.max_height_above_ground = g.max_camera_height;
        self.min_height_above_ground = g.min_camera_height;
        self.ok_to_adjust_height = false;
        self.default_angle = 0.0;
        self.default_pitch_angle = 0.0;
        // W3DView: the default "lookat" point
        self.pos = Coord3D { x: 87.0f32 * MAP_XY_FACTOR, y: 77.0f32 * MAP_XY_FACTOR, z: 0.0 };
        self.camera = ViewCamera::new();
        self.set_camera_transform(h);
        self.camera_constraint_valid = false;
        self.scroll_amount_cutoff = h.global().scroll_amount_cutoff;
    }

    /// `W3DView::setWidth(width)`: the camera's aspect, viewport and a 50 degree view plane scaled to the display.
    pub fn set_width(&mut self, width: i32, display_width: i32) {
        self.width = width;
        self.camera.set_aspect_ratio(width as f32 / self.height as f32);
        self.camera.viewport.1.x = (self.origin.x + width) as f32 / display_width as f32;
        self.camera.set_view_plane_fov(width as f32 / display_width as f32 * deg_to_radf(50.0), -1.0);
    }

    /// `W3DView::setHeight(height)`: the camera's aspect and viewport.
    pub fn set_height(&mut self, height: i32, display_height: i32) {
        self.height = height;
        self.camera.set_aspect_ratio(self.width as f32 / height as f32);
        self.camera.viewport.1.y = (self.origin.y + height) as f32 / display_height as f32;
    }

    /// `View::setMouseLock(locked)`.
    pub fn set_mouse_lock(&mut self, locked: bool) {
        self.mouse_locked = locked;
    }

    /// `View::isMouseLocked()`.
    pub fn is_mouse_locked(&self) -> bool {
        self.mouse_locked
    }

    /// `View::setOkToAdjustHeight(val)`.
    pub fn set_ok_to_adjust_height(&mut self, val: bool) {
        self.ok_to_adjust_height = val;
    }

    /// `buildCameraTransform(&transform)`: the camera offset scaled by the zoom, pitched, turned, pulled in to the
    /// ground level and moved over the look-at point (kept inside the constraint once it is valid), looking at the
    /// point on the ground level. The non-real-zoom branch; no shake.
    pub fn build_camera_transform(&self) -> Matrix3D {
        let ground_level = self.ground_level;
        let zoom = self.zoom;
        let angle = self.angle;
        let pitch = self.pitch_angle;
        let mut pos = self.pos;
        if self.camera_constraint_valid {
            pos.x = maxf(self.camera_constraint.lo.x, pos.x);
            pos.x = minf(self.camera_constraint.hi.x, pos.x);
            pos.y = maxf(self.camera_constraint.lo.y, pos.y);
            pos.y = minf(self.camera_constraint.hi.y, pos.y);
        }
        let mut source = Vector3::new(self.camera_offset.x * zoom, self.camera_offset.y * zoom, self.camera_offset.z * zoom);
        let mut target = Vector3::new(0.0, 0.0, 0.0);
        let factor = (1.0f64 - f64::from(ground_level / source.z)) as f32;
        let angle_transform = Matrix3D::from_axis_angle(&Vector3::new(0.0, 0.0, 1.0), angle);
        let pitch_transform = Matrix3D::from_axis_angle(&Vector3::new(1.0, 0.0, 0.0), pitch);
        source = pitch_transform.mul_vector3(&source);
        source = angle_transform.mul_vector3(&source);
        source *= factor;
        source.x += pos.x;
        source.y += pos.y;
        source.z += ground_level;
        target.x += pos.x;
        target.y += pos.y;
        target.z += ground_level;
        // do fxPitch adjustment (not the real zoom camera)
        if self.fx_pitch <= 1.0f32 {
            let mut height = source.z - target.z;
            height *= self.fx_pitch;
            target.z = source.z - height;
        } else {
            source.x = target.x + ((source.x - target.x) / self.fx_pitch);
            source.y = target.y + ((source.y - target.y) / self.fx_pitch);
        }
        let mut transform = Matrix3D::identity();
        transform.look_at(&source, &target, 0.0);
        transform
    }

    /// The camera's transform as `setCameraTransform` last set it (`m_3DCamera->Get_Transform()`): the eye is its
    /// translation, and it looks along its -Z axis. The client places its camera by it.
    pub fn camera_transform(&self) -> Matrix3D {
        self.camera.transform
    }

    /// `getPickRay(&screen, &start, &end)`: from the camera through a pixel of the view to the far clip plane.
    pub fn get_pick_ray(&self, screen: ICoord2D) -> (Vector3, Vector3) {
        let (log_x, log_y) = pixel_screen_to_w3d_logical_screen(screen.x - self.origin.x, screen.y - self.origin.y, self.width, self.height);
        let ray_start = self.camera.position();
        let mut ray_end = self.camera.un_project(Vector2::new(log_x, log_y));
        ray_end -= ray_start;
        ray_end.normalize();
        ray_end *= self.camera.depth();
        ray_end += ray_start;
        (ray_start, ray_end)
    }

    /// `calcCameraConstraints()`: the map's extent less the ground distance from the screen's centre to 95 % down it
    /// (pushed out by 1000 with the AI debug on).
    pub fn calc_camera_constraints(&mut self, h: &impl ViewHost) {
        let Some(map_region) = h.extent() else { return };
        let max_edge_z = self.ground_level;
        let mut screen = ICoord2D { x: (0.5f32 * self.width as f32 + self.origin.x as f32) as i32, y: (0.5f32 * self.height as f32 + self.origin.y as f32) as i32 };
        let (s, e) = self.get_pick_ray(screen);
        let mut center = Coord3D { x: Vector3::find_x_at_z(max_edge_z, &s, &e), y: Vector3::find_y_at_z(max_edge_z, &s, &e), z: max_edge_z };
        screen.y = (self.origin.y as f32 + 0.95f32 * self.height as f32) as i32;
        let (s, e) = self.get_pick_ray(screen);
        let bottom = Coord3D { x: Vector3::find_x_at_z(max_edge_z, &s, &e), y: Vector3::find_y_at_z(max_edge_z, &s, &e), z: max_edge_z };
        center.x -= bottom.x;
        center.y -= bottom.y;
        let mut offset = center.length();
        if h.global().debug_ai != 0 {
            offset = -1000.0;
        }
        self.camera_constraint.lo.x = map_region.lo.x + offset;
        self.camera_constraint.hi.x = map_region.hi.x - offset;
        self.camera_constraint.lo.y = map_region.lo.y + offset;
        self.camera_constraint.hi.y = map_region.hi.y - offset;
        self.camera_constraint_valid = true;
    }

    /// `setCameraTransform()`: the clip planes (near `MAP_XY_FACTOR`, far 1200, ten times that when the whole terrain
    /// is drawn, the camera is pitched up or zoomed out past 1.05), the constraints made when they are not valid and the
    /// look-at point kept inside them, then the camera placed.
    pub fn set_camera_transform(&mut self, h: &impl ViewHost) {
        self.camera_has_moved_since_request = true;
        let near_z = MAP_XY_FACTOR;
        let mut far_z = 1200.0f32;
        if h.global().draw_entire_terrain || (self.fx_pitch < 0.95f32 || f64::from(self.zoom) > 1.05) {
            far_z *= MAP_XY_FACTOR;
        }
        self.camera.clip = (near_z, far_z);
        if !self.camera_constraint_valid {
            self.camera.transform = self.build_camera_transform();
            self.calc_camera_constraints(h);
        }
        if self.camera_constraint_valid {
            let mut pos = self.pos;
            pos.x = maxf(self.camera_constraint.lo.x, pos.x);
            pos.x = minf(self.camera_constraint.hi.x, pos.x);
            pos.y = maxf(self.camera_constraint.lo.y, pos.y);
            pos.y = minf(self.camera_constraint.hi.y, pos.y);
            self.pos = pos;
        }
        self.camera.transform = self.build_camera_transform();
    }

    /// `forceRedraw()`.
    pub fn force_redraw(&mut self, h: &impl ViewHost) {
        self.set_camera_transform(h);
    }

    /// `W3DView::scrollBy(&delta)`: a nonzero screen delta moves the look-at point by the world distance between the
    /// view's bottom-right pixel and that pixel plus 250 times the delta (the y scaled by the aspect) on the view plane.
    pub fn scroll_by(&mut self, delta: &Coord2D, h: &impl ViewHost) {
        if delta.x != 0.0 || delta.y != 0.0 {
            self.scroll_amount = *delta;
            let start = Vector2::new(self.width as f32, self.height as f32);
            // (`Real aspect = getWidth()/getHeight();`: an integer division of two `Int`s)
            let mut aspect = (self.width / self.height) as f32;
            if aspect == 0.0 && self.portrait_scroll {
                aspect = 1.0;
            }
            let end = Vector2::new(start.x + delta.x * SCROLL_RESOLUTION, start.y + delta.y * SCROLL_RESOLUTION * aspect);
            let (rw, rh) = self.render_target();
            let world_start = self.camera.device_to_world_space(start, rw, rh);
            let world_end = self.camera.device_to_world_space(end, rw, rh);
            let world = Vector3::new(world_end.x - world_start.x, world_end.y - world_start.y, world_end.z - world_start.z);
            let mut pos = self.pos;
            pos.x += world.x;
            pos.y += world.y;
            self.pos = pos;
            self.set_camera_transform(h);
        }
    }

    /// `WW3D::Get_Render_Target_Resolution`: the display, which the view fills from its origin.
    fn render_target(&self) -> (i32, i32) {
        let (vx, vy) = (self.camera.viewport.1.x, self.camera.viewport.1.y);
        (((self.origin.x + self.width) as f32 / vx) as i32, ((self.origin.y + self.height) as f32 / vy) as i32)
    }

    /// `View::getAngle()`.
    pub fn get_angle(&self) -> f32 {
        self.angle
    }

    /// `W3DView::setAngle(angle)`: folded into ±π, then the camera placed.
    pub fn set_angle(&mut self, angle: f32, h: &impl ViewHost) {
        self.angle = norm_angle(angle);
        self.set_camera_transform(h);
    }

    /// `View::getPitch()`.
    pub fn get_pitch(&self) -> f32 {
        self.pitch_angle
    }

    /// `W3DView::setPitch(angle)` (`View::setPitch`): clamped to ±π/5, then the camera placed.
    pub fn set_pitch(&mut self, angle: f32, h: &impl ViewHost) {
        self.pitch_angle = angle;
        let limit = PI / 5.0f32;
        if self.pitch_angle < -limit {
            self.pitch_angle = -limit;
        } else if self.pitch_angle > limit {
            self.pitch_angle = limit;
        }
        self.set_camera_transform(h);
    }

    /// `W3DView::setAngleAndPitchToDefault()`: the defaults, the FX pitch to 1, the camera placed.
    pub fn set_angle_and_pitch_to_default(&mut self, h: &impl ViewHost) {
        self.angle = self.default_angle;
        self.pitch_angle = self.default_pitch_angle;
        self.fx_pitch = 1.0;
        self.set_camera_transform(h);
    }

    /// `W3DView::setDefaultView(pitch, angle, maxHeight)`: the default pitch and the maximum height (the global data's
    /// times `max_height`, at least the minimum); the angle is ignored.
    pub fn set_default_view(&mut self, pitch: f32, _angle: f32, max_height: f32, g: &GlobalData) {
        self.default_pitch_angle = pitch;
        self.max_height_above_ground = g.max_camera_height * max_height;
        if self.min_height_above_ground > self.max_height_above_ground {
            self.max_height_above_ground = self.min_height_above_ground;
        }
    }

    /// `View::getHeightAboveGround()`.
    pub fn get_height_above_ground(&self) -> f32 {
        self.height_above_ground
    }

    /// `W3DView::setHeightAboveGround(z)`: clamped to the camera heights while the zoom is limited.
    pub fn set_height_above_ground(&mut self, z: f32, h: &impl ViewHost) {
        self.height_above_ground = z;
        if self.zoom_limited {
            if self.height_above_ground < self.min_height_above_ground {
                self.height_above_ground = self.min_height_above_ground;
            }
            if self.height_above_ground > self.max_height_above_ground {
                self.height_above_ground = self.max_height_above_ground;
            }
        }
        self.camera_constraint_valid = false;
        self.set_camera_transform(h);
    }

    /// `View::zoomIn()`: 10 lower.
    pub fn zoom_in(&mut self, h: &impl ViewHost) {
        self.set_height_above_ground(self.get_height_above_ground() - 10.0f32, h);
    }

    /// `View::zoomOut()`: 10 higher.
    pub fn zoom_out(&mut self, h: &impl ViewHost) {
        self.set_height_above_ground(self.get_height_above_ground() + 10.0f32, h);
    }

    /// `View::getZoom()`.
    pub fn get_zoom(&self) -> f32 {
        self.zoom
    }

    /// `W3DView::setZoom(z)`: clamped to the zoom limits.
    pub fn set_zoom(&mut self, z: f32, h: &impl ViewHost) {
        self.zoom = z;
        if self.zoom < self.min_zoom {
            self.zoom = self.min_zoom;
        }
        if self.zoom > self.max_zoom {
            self.zoom = self.max_zoom;
        }
        self.camera_constraint_valid = false;
        self.set_camera_transform(h);
    }

    /// `W3DView::setZoomToDefault()`: the zoom that puts the camera the maximum height above the highest ground around
    /// the look-at point (no clamp), and that maximum as the height asked for.
    pub fn set_zoom_to_default(&mut self, h: &impl ViewHost) {
        let terrain_height_max = get_height_around_pos(h, self.pos.x, self.pos.y);
        let desired_height = terrain_height_max + self.max_height_above_ground;
        let desired_zoom = desired_height / self.camera_offset.z;
        self.zoom = desired_zoom;
        self.height_above_ground = self.max_height_above_ground;
        self.camera_constraint_valid = false;
        self.set_camera_transform(h);
    }

    /// #131: `W3DView::resetCamera(location, milliseconds, easeIn, easeOut)` (W3DView.cpp:2789), as the view stands once
    /// the movements it starts have finished, then the camera placed (`updateView`'s `recalcCamera`). `InGameUI::resetCamera`
    /// passes the view's own position and 1 ms, and every 1 ms movement finishes on the next frame's
    /// `updateCameraMovements`: [`Self::reset_camera_end_state`].
    pub fn reset_camera(&mut self, location: &Coord3D, milliseconds: i32, ease_in: f32, ease_out: f32, h: &impl ViewHost) {
        self.reset_camera_end_state(location, milliseconds, ease_in, ease_out, h);
        self.set_camera_transform(h);
    }

    /// #131: what `resetCamera(location, 1, 0, 0)` and the next frame's `updateCameraMovements` leave:
    /// - `zoomCamera(desiredZoom, ...)`: the zoom that puts the camera the maximum height above the highest ground around
    ///   `location` (`getHeightAroundPos`), reached in its one frame (`zoomCameraOneFrame`'s `m_zoom = endZoom`); the
    ///   height asked for (`m_heightAboveGround`) is **not** changed, so the next frames' height adjustment
    ///   ([`Self::update_height`]) takes the zoom back toward it;
    /// - `pitchCamera(1.0, ...)`: the FX pitch 1;
    /// - `moveCameraTo(location, ...)` with `m_mcwpInfo.cameraAngle[2] = 0.0` after it: at the path's end
    ///   (`moveAlongWaypointPath`'s finish) the angle 0, the ground level the ground under `location` (the path's last
    ///   `groundHeight`, the blend `m_groundLevel*0 + newGround*1`), the camera offset's y and x from its z and the
    ///   global data's pitch and yaw, the position `location` at height 0, and the camera constraints widened to hold it.
    ///
    /// With `TheGlobalData->m_disableCameraMovement` the movements end without moving anything. Only the instant form is
    /// translated (`milliseconds` up to one frame, `TheW3DFrameLengthInMsec`, and no ease, as `InGameUI::resetCamera`
    /// calls it); the scripted camera's frames in between are not (see the module docs). The location must be the view's
    /// own position (as `resetCamera`'s caller passes): another one within a unit of it would take an in-between frame
    /// first.
    pub fn reset_camera_end_state(&mut self, location: &Coord3D, _milliseconds: i32, _ease_in: f32, _ease_out: f32, h: &impl ViewHost) {
        // (`moveCameraTo` keeps `m_angle = m_mcwpInfo.cameraAngle[0]`, its own angle: no change)
        // "terrain height + desired height offset == cameraOffset * actual zoom"
        let terrain_height_max = get_height_around_pos(h, location.x, location.y);
        let above = if crate::logic::modules::plant(9207) { self.height_above_ground } else { self.max_height_above_ground };
        let desired_height = terrain_height_max + above;
        let desired_zoom = desired_height / self.camera_offset.z;
        let g = h.global();
        if g.disable_camera_movement {
            return;
        }
        // zoomCameraOneFrame, pitchCameraOneFrame
        self.zoom = desired_zoom;
        if !crate::logic::modules::plant(9208) {
            self.fx_pitch = 1.0;
        }
        // moveAlongWaypointPath's finish: `cameraAngle[numWaypoints]`, set to 0 by resetCamera ("default angle")
        if !crate::logic::modules::plant(9206) {
            self.angle = 0.0;
        }
        // `groundHeight[numWaypoints] = m_groundLevel*factor1 + newGround*factor2` with factor2 = 1
        let mut new_ground = h.ground_height(location.x, location.y);
        if crate::logic::modules::plant(9210) && new_ground > 120.0f32 {
            new_ground = 120.0;
        }
        let factor2 = 1.0f32;
        let factor1 = (1.0f64 - f64::from(factor2)) as f32;
        self.ground_level = self.ground_level * factor1 + new_ground * factor2;
        self.camera_offset.y = (-(f64::from(self.camera_offset.z) / petir::Real::tan(f64::from(g.camera_pitch) * (f64::from(PI) / 180.0)))) as f32;
        self.camera_offset.x = (-(f64::from(self.camera_offset.y) * petir::Real::tan(f64::from(g.camera_yaw) * (f64::from(PI) / 180.0)))) as f32;
        let mut pos = *location;
        pos.z = 0.0;
        self.pos = pos;
        // "assuming that the scripter knows what he is doing, we adjust the constraints so that the scripted action can
        // occur."
        if !crate::logic::modules::plant(9209) {
            self.camera_constraint.lo.x = minf(self.camera_constraint.lo.x, pos.x);
            self.camera_constraint.hi.x = maxf(self.camera_constraint.hi.x, pos.x);
            self.camera_constraint.lo.y = minf(self.camera_constraint.lo.y, pos.y);
            self.camera_constraint.hi.y = maxf(self.camera_constraint.hi.y, pos.y);
        }
    }

    /// `View::getLocation(&location)`.
    pub fn get_location(&self) -> ViewLocation {
        let mut l = ViewLocation::default();
        l.init(self.pos.x, self.pos.y, self.pos.z, self.get_angle(), self.get_pitch(), self.get_zoom());
        l
    }

    /// `View::setLocation(&location)`: a valid location's position, angle, pitch and zoom, then a redraw.
    pub fn set_location(&mut self, l: &ViewLocation, h: &impl ViewHost) {
        if l.valid {
            self.pos = l.pos;
            self.set_angle(l.angle, h);
            self.set_pitch(l.pitch, h);
            self.set_zoom(l.zoom, h);
            self.force_redraw(h);
        }
    }

    /// `W3DView::lookAt(&o)`: the look-at point to `o` on the ground (z 0), the camera placed. (A point higher than
    /// a pathfind cell above the ground would be cast onto the terrain: not translated, see the module.)
    pub fn look_at(&mut self, o: &Coord3D, h: &impl ViewHost) {
        let mut pos = *o;
        let _above = o.z > PATHFIND_CELL_SIZE_F + h.ground_height(pos.x, pos.y);
        pos.z = 0.0;
        self.pos = pos;
        self.set_camera_transform(h);
    }

    /// `W3DView::initHeightForMap()`: the ground level under the look-at point (at most 120) and the camera offset
    /// above it.
    pub fn init_height_for_map(&mut self, h: &impl ViewHost) {
        self.ground_level = h.ground_height(self.pos.x, self.pos.y);
        const MAX_GROUND_LEVEL: f32 = 120.0;
        if self.ground_level > MAX_GROUND_LEVEL {
            self.ground_level = MAX_GROUND_LEVEL;
        }
        let g = h.global();
        self.camera_offset.z = self.ground_level + g.camera_height;
        self.camera_offset.y = (-(f64::from(self.camera_offset.z) / petir::Real::tan(f64::from(g.camera_pitch) * (f64::from(PI) / 180.0)))) as f32;
        self.camera_offset.x = (-(f64::from(self.camera_offset.y) * petir::Real::tan(f64::from(g.camera_yaw) * (f64::from(PI) / 180.0)))) as f32;
        self.camera_constraint_valid = false;
        self.set_camera_transform(h);
    }

    /// `W3DView::update()`'s height above ground: while the view may adjust its height and the game runs, the zoom
    /// moves a `m_cameraAdjustSpeed` share toward the height asked for above the ground under the camera; while
    /// scrolling only when the scroll is slow or the camera is too low or (enforced) too high. The camera is placed when
    /// the zoom moved. `ui_scrolling` is `TheInGameUI->isScrolling()`. (No scripted movement: `didScriptedMovement` is
    /// false; the replay camera is not used.)
    pub fn update_height(&mut self, ui_scrolling: bool, h: &impl ViewHost) {
        let mut recalc_camera = false;
        let did_scripted_movement = false;
        self.terrain_height_under_camera = get_height_around_pos(h, self.pos.x, self.pos.y);
        self.current_height_above_ground = self.camera_offset.z * self.zoom - self.terrain_height_under_camera;
        if self.ok_to_adjust_height && !h.game_paused() {
            let g = h.global();
            let desired_height = self.terrain_height_under_camera + self.height_above_ground;
            let desired_zoom = desired_height / self.camera_offset.z;
            if ui_scrolling {
                if self.scroll_amount.length() < self.scroll_amount_cutoff
                    || (self.current_height_above_ground < self.min_height_above_ground)
                    || (g.enforce_max_camera_height && self.current_height_above_ground > self.max_height_above_ground)
                {
                    let zoom_adj = (desired_zoom - self.zoom) * g.camera_adjust_speed;
                    if f64::from(crate::ww::wwmath::fabs(zoom_adj)) >= 0.0001 {
                        self.zoom += zoom_adj;
                        recalc_camera = true;
                    }
                }
            } else {
                let zoom_adj = (self.zoom - desired_zoom) * g.camera_adjust_speed;
                let zoom_adj_abs = crate::ww::wwmath::fabs(zoom_adj);
                if f64::from(zoom_adj_abs) >= 0.0001 && !did_scripted_movement {
                    self.zoom -= zoom_adj;
                    recalc_camera = true;
                }
            }
        }
        if recalc_camera {
            self.set_camera_transform(h);
        }
    }
}

/// #131: `TheTacticalView` for `InGameUI::update`'s keyboard camera: the view with its host.
pub struct HostedView<'a, H: ViewHost> {
    pub view: &'a mut TacticalView,
    pub h: &'a H,
}

impl<H: ViewHost> crate::client::in_game_ui::KeyboardCameraView for HostedView<'_, H> {
    fn get_angle(&self) -> f32 {
        self.view.get_angle()
    }
    fn set_angle(&mut self, angle: f32) {
        self.view.set_angle(angle, self.h);
    }
    fn zoom_in(&mut self) {
        self.view.zoom_in(self.h);
    }
    fn zoom_out(&mut self) {
        self.view.zoom_out(self.h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat map 0 .. 1000 square at height `ground`, with the shipped-looking camera fields set by the test (not the
    /// game's files: values picked for the test).
    struct Flat {
        g: GlobalData,
        ground: f32,
        paused: bool,
    }

    impl ViewHost for Flat {
        fn global(&self) -> &GlobalData {
            &self.g
        }
        fn ground_height(&self, x: f32, _y: f32) -> f32 {
            // a ridge at x > 2000 for getHeightAroundPos
            if x > 4000.0 { self.ground + 50.0 } else { self.ground }
        }
        fn extent(&self) -> Option<Region3D> {
            Some(Region3D { lo: Coord3D { x: 0.0, y: 0.0, z: 0.0 }, hi: Coord3D { x: 5000.0, y: 5000.0, z: 0.0 } })
        }
        fn game_paused(&self) -> bool {
            self.paused
        }
    }

    fn host() -> Flat {
        let mut g = GlobalData::new();
        g.camera_pitch = 40.0;
        g.camera_height = 250.0;
        g.max_camera_height = 300.0;
        g.min_camera_height = 100.0;
        Flat { g, ground: 0.0, paused: false }
    }

    /// #131, predicted before the run: the view turned to 1.2, its FX pitch 0.5 and zoomed in, `InGameUI::resetCamera`
    /// leaves it at the same point with angle 0, FX pitch 1, the ground level of the ground there, and the zoom of the
    /// maximum height over the highest ground around it (`setZoomToDefault`'s zoom at that point) while the height asked
    /// for stays the zoomed-in one; with camera movement disabled nothing changes.
    #[test]
    fn the_camera_reset_turns_to_north_and_zooms_out_where_it_looks() {
        let h = host();
        let mut v = started(&h);
        v.set_angle(1.2, &h);
        v.fx_pitch = 0.5;
        v.zoom_in(&h);
        let asked = v.height_above_ground;
        let at = v.pos;
        crate::client::in_game_ui::reset_camera(&mut v, &h);
        let mut d = started(&h);
        d.pos = at;
        d.set_zoom_to_default(&h);
        assert_eq!((v.pos.x, v.pos.y, v.angle, v.fx_pitch), (at.x, at.y, 0.0, 1.0));
        assert_eq!((v.zoom, v.ground_level, v.height_above_ground), (d.zoom, 0.0, asked));
        let mut off = host();
        off.g.disable_camera_movement = true;
        let mut w = started(&off);
        w.set_angle(1.2, &off);
        crate::client::in_game_ui::reset_camera(&mut w, &off);
        assert_eq!(w.angle, norm_angle(1.2));
    }

    /// The game's start sequence (`InGameUI::init`, `GameLogic::startNewGame`) on a 800x600 display.
    fn started(h: &Flat) -> TacticalView {
        let mut v = TacticalView::new(&h.g);
        v.init(h);
        v.set_width(800, 800);
        v.set_height(600, 600);
        v.set_default_view(0.0, 0.0, 1.0, &h.g);
        v.look_at(&Coord3D { x: 2500.0, y: 2500.0, z: 0.0 }, h);
        v.init_height_for_map(h);
        v.set_angle_and_pitch_to_default(h);
        v.set_zoom_to_default(h);
        v.set_ok_to_adjust_height(true);
        v
    }

    #[test]
    fn norm_angle_folds_and_zeroes() {
        assert_eq!(norm_angle(0.5), 0.5);
        assert!((norm_angle(3.0 * PI) - PI).abs() < 1e-5 || (norm_angle(3.0 * PI) + PI).abs() < 1e-5);
        assert_eq!(norm_angle(11.0 * PI), 0.0);
        assert_eq!(norm_angle(-11.0 * PI), 0.0);
        assert!(norm_angle(-4.0).abs() <= PI);
    }

    #[test]
    fn camera_offset_from_the_global_data() {
        let h = host();
        let v = TacticalView::new(&h.g);
        // y = -(250 / tan(40 deg)) = -297.9..
        assert!((v.camera_offset.y + 297.94).abs() < 0.05, "{}", v.camera_offset.y);
        assert_eq!(v.camera_offset.x, 0.0);
        assert_eq!(v.camera_offset.z, 250.0);
    }

    /// The start sequence: the zoom puts the camera at the maximum height (zoom 300/250 over flat ground at level 0),
    /// the camera south of the point (y lower), looking north and down.
    #[test]
    fn start_places_the_camera_over_the_point() {
        let h = host();
        let v = started(&h);
        assert_eq!(v.zoom, 1.2);
        assert_eq!(v.height_above_ground, 300.0);
        assert_eq!(v.ground_level, 0.0);
        let eye = v.camera_transform().get_translation();
        assert!((eye.x - 2500.0).abs() < 1e-2, "{eye:?}");
        assert!(eye.y < 2500.0 && eye.z > 250.0, "{eye:?}");
        // the camera's -Z looks down toward +y
        let fwd = v.camera_transform().get_z_vector();
        assert!(fwd.y < 0.0 && fwd.z > 0.0, "{fwd:?}");
        assert!(v.camera_constraint_valid);
        assert!(v.camera_constraint.lo.x > 0.0 && v.camera_constraint.hi.x < 5000.0);
    }

    /// zoomIn lowers the height asked for by 10 and update settles the zoom toward it; the limits clamp it.
    #[test]
    fn zoom_in_and_out_settle_through_update() {
        let h = host();
        let mut v = started(&h);
        v.zoom_in(&h);
        assert_eq!(v.height_above_ground, 290.0);
        let z0 = v.zoom;
        v.update_height(false, &h);
        assert!(v.zoom < z0);
        for _ in 0..200 {
            v.update_height(false, &h);
        }
        assert!((v.zoom - 290.0 / 250.0).abs() < 1e-3, "{}", v.zoom);
        for _ in 0..40 {
            v.zoom_in(&h);
        }
        assert_eq!(v.height_above_ground, 100.0);
        for _ in 0..40 {
            v.zoom_out(&h);
        }
        assert_eq!(v.height_above_ground, 300.0);
        // paused: no settling
        let mut p = host();
        p.paused = true;
        for _ in 0..5 {
            v.zoom_in(&p);
        }
        let z = v.zoom;
        v.update_height(false, &p);
        assert_eq!(v.zoom, z);
        // scrolling fast at a height in range: no settling either
        v.scroll_amount = Coord2D { x: 100.0, y: 0.0 };
        v.update_height(true, &h);
        assert_eq!(v.zoom, z);
        // scrolling slowly: settles
        v.scroll_amount = Coord2D { x: 1.0, y: 0.0 };
        v.update_height(true, &h);
        assert_ne!(v.zoom, z);
    }

    /// scrollBy: right moves the point to +x, down moves it to -y (the camera looks north), and records the amount.
    #[test]
    fn scroll_by_moves_the_point_on_the_view_plane() {
        let h = host();
        let mut v = started(&h);
        let p0 = v.pos;
        v.scroll_by(&Coord2D { x: 0.1, y: 0.0 }, &h);
        assert!(v.pos.x > p0.x && (v.pos.y - p0.y).abs() < 1e-2, "{:?} {:?}", p0, v.pos);
        assert_eq!(v.scroll_amount, Coord2D { x: 0.1, y: 0.0 });
        let p1 = v.pos;
        v.scroll_by(&Coord2D { x: 0.0, y: 0.1 }, &h);
        assert!(v.pos.y < p1.y, "{:?} {:?}", p1, v.pos);
        // a zero delta does nothing
        let p2 = v.pos;
        v.scroll_by(&Coord2D::default(), &h);
        assert_eq!(v.pos, p2);
        // far past the map's edge: held inside the constraint
        // (a delta of 1 moves the point about 0.29 world units here: 250 pixels of 800 across a view plane 2 tan 25
        // degrees wide at distance 1)
        for _ in 0..500 {
            v.scroll_by(&Coord2D { x: 100.0, y: 0.0 }, &h);
        }
        assert_eq!(v.pos.x, v.camera_constraint.hi.x);
        // a display taller than wide: the integer aspect is 0 and a vertical delta does nothing, unless the client's
        // portrait_scroll is set
        v.set_width(390, 390);
        v.set_height(844, 844);
        let p3 = v.pos;
        v.scroll_by(&Coord2D { x: 0.0, y: 10.0 }, &h);
        assert_eq!(v.pos, p3);
        v.portrait_scroll = true;
        v.scroll_by(&Coord2D { x: 0.0, y: 10.0 }, &h);
        assert!(v.pos.y < p3.y, "{:?} {:?}", p3, v.pos);
    }

    /// The angle folds, the pitch clamps to ±π/5, the defaults restore both; locations save and restore the view.
    #[test]
    fn angle_pitch_and_locations() {
        let h = host();
        let mut v = started(&h);
        v.set_angle(4.0, &h);
        assert!((v.get_angle() - (4.0 - 2.0 * PI)).abs() < 1e-5);
        v.set_pitch(2.0, &h);
        assert_eq!(v.get_pitch(), PI / 5.0);
        v.set_pitch(-2.0, &h);
        assert_eq!(v.get_pitch(), -PI / 5.0);
        let saved = v.get_location();
        assert!(saved.valid);
        v.set_angle_and_pitch_to_default(&h);
        assert_eq!((v.get_angle(), v.get_pitch(), v.fx_pitch), (0.0, 0.0, 1.0));
        v.set_zoom(5.0, &h);
        assert_eq!(v.get_zoom(), 1.3);
        v.set_zoom(0.0, &h);
        assert_eq!(v.get_zoom(), 0.2);
        v.set_location(&saved, &h);
        assert_eq!((v.get_angle(), v.get_pitch(), v.get_zoom()), (saved.angle, saved.pitch, saved.zoom));
        // an invalid location is ignored
        let before = v.clone();
        v.set_location(&ViewLocation::default(), &h);
        assert_eq!(v, before);
        v.set_mouse_lock(true);
        assert!(v.is_mouse_locked());
    }

    /// getHeightAroundPos takes the highest of its five samples; setDefaultView keeps the maximum at least the minimum.
    #[test]
    fn height_around_and_default_view() {
        let h = host();
        assert_eq!(get_height_around_pos(&h, 3990.0, 0.0), 50.0);
        assert_eq!(get_height_around_pos(&h, 100.0, 0.0), 0.0);
        let mut v = started(&h);
        v.set_default_view(0.3, 1.0, 0.1, &h.g);
        assert_eq!(v.max_height_above_ground, 100.0);
        assert_eq!(v.default_pitch_angle, 0.3);
        assert_eq!(v.default_angle, 0.0);
        assert_eq!(pixel_screen_to_w3d_logical_screen(0, 0, 800, 600), (-1.0, 1.0));
    }
}
