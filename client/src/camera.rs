// SPDX-License-Identifier: GPL-3.0-only
//! The tactical view's camera under the player's input (GitHub #119): the translated `LookAtTranslator`
//! (`rts_lockstep_engine::client::look_at_xlat`) on the translated view (`rts_lockstep_engine::client::tactical_view`),
//! and the InGameUI members they share (`m_isScrolling`, `m_scrollAmount`, the RMB anchor option, the keyboard camera
//! flags). Plain Rust, no Bevy: the application gives each raw input to [`Camera::translate`] and runs
//! [`Camera::tick`] at the client's frame rate, then places its 3D camera where [`Camera::eye_target`] says.
//! ~~(The application's `camera_input` stood in for the translator, with no test.)~~ **Corrected 2026-11-10 (#119):**
//! it now only gives the translator the raw input.
//!
//! What is the game's, and what is the client's:
//!
//! - **Translated:** the translator and the view (see their modules); `InGameUI::update`'s keyboard rotation and zoom
//!   ([`CameraKeys::update`]); the game's start of the view (`InGameUI::init`'s size and default view,
//!   `GameLogic::startNewGame`'s `lookAt`, `initHeightForMap`, `setAngleAndPitchToDefault`, `setZoomToDefault`,
//!   `setOkToAdjustHeight(TRUE)`: [`Camera::start`]).
//! - **The client's (new):** the client frame is 30 a second ([`Camera::ticks`], the game's own frame cap); the camera's
//!   global data is the World's, or for the demo (which has no `GameData.ini`) a pitch, a height and limits made from
//!   the map's size ([`camera_globals`]); a display taller than wide scrolls vertically with the aspect 1
//!   (`TacticalView::portrait_scroll`); `MSG_SET_REPLAY_CAMERA` is not appended (the client records no replay; the
//!   global data's `m_saveCameraInReplay` is off by default); the bookmark's message is not shown (no message area yet).
//! - The simulation is never changed: the camera reads the World's terrain, frame and pause and writes nothing to it.

use rts_lockstep_engine::client::in_game_ui::InGameUiSelection;
use rts_lockstep_engine::client::look_at_xlat::{LookAtGlobals, LookAtHost, LookAtTranslator, MOUSE_ARROW};
use rts_lockstep_engine::client::selection_xlat::Disposition;
use rts_lockstep_engine::client::tactical_view::{TacticalView, ViewHost, ViewLocation};
use rts_lockstep_engine::client::window_xlat::RawMessage;
use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D, ICoord2D, Region3D};
use rts_lockstep_engine::common::global_data::GlobalData;
use rts_lockstep_engine::common::message_types::MSG_FRAME_TICK;
use rts_lockstep_engine::common::recorder::game_mode;
use rts_lockstep_engine::logic::world::game_logic::World;

/// The client's frames a second (the game caps its frame rate at 30).
pub const CLIENT_HZ: f32 = 30.0;

/// The global data the camera reads: the World's (`TheGlobalData`, from `GameData.ini`), else the defaults; with no
/// camera height or pitch (the demo, no `GameData.ini`) the client's own: a 54.4 degree pitch (the C1 camera's 0.95
/// radian) and a height and maximum of a fifth of the map's size `size`, the minimum 0.08 of it (a small map seen as
/// the game sees its own large ones: the view's constraint keeps the point a screen's reach inside the map's edges, so
/// a camera that sees the whole map cannot move). New, not the game's.
pub fn camera_globals(world: &World, size: f32) -> GlobalData {
    let mut g = world.statics.global_data.the_global_data().cloned().unwrap_or_else(GlobalData::new);
    if g.camera_height <= 0.0 || g.camera_pitch <= 0.0 {
        g.camera_pitch = 54.43;
        g.camera_yaw = 0.0;
        g.camera_height = size * 0.2;
        g.max_camera_height = size * 0.2;
        g.min_camera_height = size * 0.08;
    }
    g
}

/// The view's host: the World's terrain, the camera's global data and the game's pause.
pub struct ViewWorld<'a> {
    pub g: &'a GlobalData,
    pub world: &'a World,
}

impl ViewHost for ViewWorld<'_> {
    fn global(&self) -> &GlobalData {
        self.g
    }
    fn ground_height(&self, x: f32, y: f32) -> f32 {
        self.world.terrain.ground_height(x, y, false).0
    }
    fn extent(&self) -> Option<Region3D> {
        self.world.terrain.extent().ok()
    }
    fn game_paused(&self) -> bool {
        self.world.game_logic.game_paused
    }
}

/// `InGameUI`'s keyboard camera flags (`m_cameraRotatingLeft`, ...), set while the client's rotate and zoom keys are
/// held (upstream: the meta keys' messages through `CommandTranslator`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CameraKeys {
    pub rotating_left: bool,
    pub rotating_right: bool,
    pub zooming_in: bool,
    pub zooming_out: bool,
}

impl CameraKeys {
    /// `InGameUI::update`'s keyboard camera: one key of a pair turns the view by `m_keyboardCameraRotateSpeed` or zooms
    /// it one step.
    pub fn update(&self, view: &mut TacticalView, h: &impl ViewHost) {
        let speed = h.global().keyboard_camera_rotate_speed;
        if self.rotating_left && !self.rotating_right {
            view.set_angle(view.get_angle() - speed, h);
        }
        if self.rotating_right && !self.rotating_left {
            view.set_angle(view.get_angle() + speed, h);
        }
        if self.zooming_in && !self.zooming_out {
            view.zoom_in(h);
        }
        if self.zooming_out && !self.zooming_in {
            view.zoom_out(h);
        }
    }
}

/// The camera: the view, the translator and the InGameUI's scroll state.
#[derive(Clone, Debug)]
pub struct Camera {
    pub view: TacticalView,
    pub look: LookAtTranslator,
    /// The camera's global data ([`camera_globals`]).
    pub globals: GlobalData,
    /// `InGameUI::m_isScrolling`.
    pub ui_scrolling: bool,
    /// `InGameUI::m_scrollAmount`.
    pub scroll_amount: Coord2D,
    /// `InGameUI::m_moveRMBScrollAnchor` (off, the InGameUI's default, until the options set it).
    pub move_rmb_anchor: bool,
    /// The keyboard camera flags.
    pub keys: CameraKeys,
    /// ~~`TheMouse`'s cursor as the translator sets it (the client draws the system's).~~ **Corrected 2026-11-10
    /// (#117):** `TheMouse->getMouseCursor()` as the owner copied it from the client's mouse
    /// (`rts_lockstep_engine::client::mouse::Mouse`) before the call, kept up to date by the calls below.
    pub cursor: i32,
    /// (#117) The calls the translator made on the cursor, in order, for the owner to make on its mouse and clear:
    /// [`CursorCall::InGameUi`] is `InGameUI::setMouseCursor` (from `InGameUI::setScrolling`), [`CursorCall::Mouse`]
    /// `TheMouse->setCursor`.
    pub cursor_calls: Vec<CursorCall>,
    /// `TheGameClient->getFrame()`.
    pub client_frame: u32,
    /// The display's size in pixels.
    pub screen: (i32, i32),
    /// Where the reset button looks (the start's point).
    pub home: Coord3D,
    /// Seconds not yet given to a client frame.
    clock: f32,
}

/// (#117) One call on the cursor made while the camera translated ([`Camera::cursor_calls`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorCall {
    /// `TheInGameUI->setMouseCursor(c)` (`rts_lockstep_engine::client::mouse::in_game_ui_set_mouse_cursor`).
    InGameUi(i32),
    /// `TheMouse->setCursor(c)`.
    Mouse(i32),
}

/// The translator's host: the camera's members, the InGameUI's selection model and the World.
struct LookWorld<'a> {
    view: &'a mut TacticalView,
    g: &'a GlobalData,
    world: &'a World,
    ui: &'a InGameUiSelection,
    ui_scrolling: &'a mut bool,
    scroll_amount: &'a mut Coord2D,
    move_rmb_anchor: bool,
    cursor: &'a mut i32,
    cursor_calls: &'a mut Vec<CursorCall>,
    client_frame: u32,
    screen: (i32, i32),
    shell_active: bool,
}

impl LookAtHost for LookWorld<'_> {
    fn ui_input_enabled(&self) -> bool {
        self.ui.input_enabled()
    }
    fn ui_is_selecting(&self) -> bool {
        self.ui.is_selecting()
    }
    fn ui_set_scrolling(&mut self, b: bool) {
        // #117: `InGameUI::setScrolling`: when it changes, `setMouseCursor(SCROLL)` on, `setMouseCursor(ARROW)` off (the
        // capture and the camera locks it also breaks are not the client's)
        use rts_lockstep_engine::client::mouse::cursor;
        if *self.ui_scrolling == b {
            return;
        }
        let c = if b { cursor::SCROLL } else { cursor::ARROW };
        *self.cursor = c;
        self.cursor_calls.push(CursorCall::InGameUi(c));
        *self.ui_scrolling = b;
    }
    fn ui_is_scrolling(&self) -> bool {
        *self.ui_scrolling
    }
    fn ui_set_scroll_amount(&mut self, amt: Coord2D) {
        *self.scroll_amount = amt;
    }
    fn ui_should_move_rmb_scroll_anchor(&self) -> bool {
        self.move_rmb_anchor
    }
    fn ui_message_bookmark_set(&mut self, _slot: i32) {}
    fn mouse_cursor(&self) -> i32 {
        *self.cursor
    }
    fn mouse_set_cursor(&mut self, cursor: i32) {
        *self.cursor = cursor;
        self.cursor_calls.push(CursorCall::Mouse(cursor));
    }
    fn shell_active(&self) -> bool {
        self.shell_active
    }
    fn stats_collector(&self) -> bool {
        false
    }
    fn stats_start_scroll_time(&mut self) {}
    fn stats_end_scroll_time(&mut self) {}
    fn logic_frame(&self) -> u32 {
        self.world.frame()
    }
    fn client_frame(&self) -> u32 {
        self.client_frame
    }
    fn display_width(&self) -> u32 {
        self.screen.0 as u32
    }
    fn display_height(&self) -> u32 {
        self.screen.1 as u32
    }
    fn globals(&self) -> LookAtGlobals {
        LookAtGlobals {
            // (the client's window is a page or a desktop window: never the game's full screen edge scroll)
            windowed: true,
            horizontal_scroll_speed_factor: self.g.horizontal_scroll_speed_factor,
            vertical_scroll_speed_factor: self.g.vertical_scroll_speed_factor,
            keyboard_scroll_factor: self.g.keyboard_scroll_factor,
            save_camera_in_replay: self.g.save_camera_in_replay,
        }
    }
    fn in_single_player_game(&self) -> bool {
        self.world.cuts.game_loop.game_mode == game_mode::SINGLE_PLAYER
    }
    fn in_skirmish_game(&self) -> bool {
        self.world.cuts.game_loop.game_mode == game_mode::SKIRMISH
    }
    fn append_replay_camera(&mut self, _loc: &ViewLocation, _cursor: i32, _pos: ICoord2D) {}
    fn view_set_mouse_lock(&mut self, b: bool) {
        self.view.set_mouse_lock(b);
    }
    fn view_set_angle_and_pitch_to_default(&mut self) {
        let vh = ViewWorld { g: self.g, world: self.world };
        self.view.set_angle_and_pitch_to_default(&vh);
    }
    fn view_set_zoom_to_default(&mut self) {
        let vh = ViewWorld { g: self.g, world: self.world };
        self.view.set_zoom_to_default(&vh);
    }
    fn view_angle(&self) -> f32 {
        self.view.get_angle()
    }
    fn view_set_angle(&mut self, a: f32) {
        let vh = ViewWorld { g: self.g, world: self.world };
        self.view.set_angle(a, &vh);
    }
    fn view_pitch(&self) -> f32 {
        self.view.get_pitch()
    }
    fn view_set_pitch(&mut self, a: f32) {
        let vh = ViewWorld { g: self.g, world: self.world };
        self.view.set_pitch(a, &vh);
    }
    fn view_zoom_in(&mut self) {
        let vh = ViewWorld { g: self.g, world: self.world };
        self.view.zoom_in(&vh);
    }
    fn view_zoom_out(&mut self) {
        let vh = ViewWorld { g: self.g, world: self.world };
        self.view.zoom_out(&vh);
    }
    fn view_scroll_by(&mut self, offset: &Coord2D) {
        let vh = ViewWorld { g: self.g, world: self.world };
        self.view.scroll_by(offset, &vh);
    }
    fn view_get_location(&mut self) -> ViewLocation {
        self.view.get_location()
    }
    fn view_set_location(&mut self, loc: &ViewLocation) {
        let vh = ViewWorld { g: self.g, world: self.world };
        self.view.set_location(loc, &vh);
    }
}

impl Camera {
    /// The camera of a new game on `world`, looking at `centre` on a display `screen` pixels across: [`Camera::start`].
    pub fn new(world: &World, centre: Coord3D, size: f32, screen: (i32, i32)) -> Self {
        let globals = camera_globals(world, size);
        let mut c = Camera {
            view: TacticalView::new(&globals),
            look: LookAtTranslator::new(),
            globals,
            ui_scrolling: false,
            scroll_amount: Coord2D::default(),
            move_rmb_anchor: false,
            keys: CameraKeys::default(),
            cursor: MOUSE_ARROW,
            cursor_calls: Vec::new(),
            client_frame: 0,
            screen,
            home: centre,
            clock: 0.0,
        };
        c.start(world, centre);
        c
    }

    /// The game's start of the view: `TheTacticalView->init()`, its size (the whole display), `setDefaultView(0, 0, 1)`,
    /// `lookAt(centre)`, `initHeightForMap()`, `setAngleAndPitchToDefault()`, `setZoomToDefault()`,
    /// `setOkToAdjustHeight(TRUE)`.
    pub fn start(&mut self, world: &World, centre: Coord3D) {
        let vh = ViewWorld { g: &self.globals, world };
        self.view.init(&vh);
        self.view.set_width(self.screen.0, self.screen.0);
        self.view.set_height(self.screen.1, self.screen.1);
        self.view.portrait_scroll = self.screen.0 < self.screen.1;
        self.view.set_default_view(0.0, 0.0, 1.0, &self.globals);
        self.view.look_at(&centre, &vh);
        self.view.init_height_for_map(&vh);
        self.view.set_angle_and_pitch_to_default(&vh);
        self.view.set_zoom_to_default(&vh);
        self.view.set_ok_to_adjust_height(true);
        self.home = centre;
    }

    /// The display's size changed: the view takes it (`Display::setDisplayMode`'s `setWidth` / `setHeight`).
    pub fn set_screen(&mut self, screen: (i32, i32), world: &World) {
        if screen == self.screen || screen.0 <= 0 || screen.1 <= 0 {
            return;
        }
        self.screen = screen;
        self.view.set_width(screen.0, screen.0);
        self.view.set_height(screen.1, screen.1);
        self.view.portrait_scroll = screen.0 < screen.1;
        let vh = ViewWorld { g: &self.globals, world };
        self.view.set_camera_transform(&vh);
    }

    /// The options' keyboard scroll speed as a share of the default (`m_keyboardScrollFactor`, #88).
    pub fn set_keyboard_scroll(&mut self, ratio: f32) {
        self.globals.keyboard_scroll_factor = self.globals.keyboard_default_scroll_factor * ratio;
    }

    /// A raw input message through `LookAtTranslator`. `shell_active`: the shell or an in-game menu has the input.
    pub fn translate(&mut self, msg: &RawMessage, ui: &InGameUiSelection, world: &World, shell_active: bool) -> Disposition {
        let Camera { view, look, globals, ui_scrolling, scroll_amount, move_rmb_anchor, cursor, cursor_calls, client_frame, screen, .. } = self;
        let mut h = LookWorld { view, g: globals, world, ui, ui_scrolling, scroll_amount, move_rmb_anchor: *move_rmb_anchor, cursor, cursor_calls, client_frame: *client_frame, screen: *screen, shell_active };
        look.translate_game_message(msg, &mut h)
    }

    /// How many client frames `dt` seconds make (30 a second; the rest is kept for the next call).
    pub fn ticks(&mut self, dt: f32) -> u32 {
        self.clock += dt.max(0.0);
        let n = (self.clock * CLIENT_HZ) as u32;
        self.clock -= n as f32 / CLIENT_HZ;
        // (a long stall does not scroll for seconds at once)
        n.min(4)
    }

    /// One client frame: `MSG_FRAME_TICK` through the translator, the InGameUI's keyboard camera, and the view's height
    /// above the ground (`W3DView::update`).
    pub fn tick(&mut self, ui: &InGameUiSelection, world: &World, shell_active: bool) {
        self.client_frame = self.client_frame.wrapping_add(1);
        self.translate(&RawMessage::mouse(MSG_FRAME_TICK, ICoord2D::default()), ui, world, shell_active);
        let vh = ViewWorld { g: &self.globals, world };
        if ui.input_enabled() {
            self.keys.update(&mut self.view, &vh);
        }
        self.view.update_height(self.ui_scrolling, &vh);
    }

    /// `ScriptInputCall::LookAtResetModes`: `TheLookAtTranslator->resetModes()`.
    pub fn reset_modes(&mut self) {
        self.look.reset_modes();
    }

    /// The reset button: the default angle, pitch and zoom (the middle button's click) at the start's point.
    pub fn reset(&mut self, world: &World) {
        let vh = ViewWorld { g: &self.globals, world };
        self.view.look_at(&self.home, &vh);
        self.view.set_angle_and_pitch_to_default(&vh);
        self.view.set_zoom_to_default(&vh);
    }

    /// The radar's look: `TheTacticalView->lookAt(&world)`.
    pub fn look_at(&mut self, at: Coord3D, world: &World) {
        let vh = ViewWorld { g: &self.globals, world };
        self.view.look_at(&at, &vh);
    }

    /// A loaded save's view (`View::xfer`: `setAngle`, `lookAt`).
    pub fn set_saved_view(&mut self, angle: f32, at: Coord3D, world: &World) {
        let vh = ViewWorld { g: &self.globals, world };
        self.view.set_angle(angle, &vh);
        self.view.look_at(&at, &vh);
    }

    /// The camera's eye and a point it looks at, game coordinates (z up): the transform's translation, and one unit
    /// along its -Z axis.
    pub fn eye_target(&self) -> ([f32; 3], [f32; 3]) {
        let m = self.view.camera_transform();
        let eye = m.get_translation();
        let z = m.get_z_vector();
        ([eye.x, eye.y, eye.z], [eye.x - z.x, eye.y - z.y, eye.z - z.z])
    }

    /// The camera's vertical field of view, radians, for a perspective projection of the view plane (`WWMath`'s `atan`, through
    /// petir), and its near and far clip planes.
    pub fn projection(&self) -> (f32, f32, f32) {
        let half = self.view.camera.view_plane.1.y;
        let fov = 2.0 * rts_lockstep_engine::ww::wwmath::atan(half);
        (fov, self.view.camera.clip.0, self.view.camera.clip.1)
    }

    /// The audio's view of the camera (`TheTacticalView`'s `getPosition()`, `getAngle()`, `get3DCameraPosition()`).
    pub fn listener(&self) -> rts_lockstep_engine::client::game_audio::ListenerView {
        let (eye, _) = self.eye_target();
        rts_lockstep_engine::client::game_audio::ListenerView {
            ground: Coord3D { x: self.view.pos.x, y: self.view.pos.y, z: 0.0 },
            angle: self.view.angle,
            camera: Coord3D { x: eye[0], y: eye[1], z: eye[2] },
        }
    }

    /// The point looked at (`m_pos`), the heading (`m_angle`, 0 looking along +y) and the eye's distance from the point.
    pub fn focus(&self) -> (Coord3D, f32, f32) {
        let (eye, _) = self.eye_target();
        let p = self.view.pos;
        let (dx, dy, dz) = (eye[0] - p.x, eye[1] - p.y, eye[2] - self.view.ground_level);
        (p, self.view.angle, rts_lockstep_engine::ww::wwmath::sqrt(dx * dx + dy * dy + dz * dz))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Controls;
    use rts_lockstep_engine::client::window_manager::key;
    use rts_lockstep_engine::common::message_types::*;
    use rts_lockstep_engine::logic::world::game_logic_update::CRC_RECALC;

    fn demo() -> (Controls, Camera) {
        let (w, _) = crate::demo::demo_world();
        let c = Controls::new(w, 1);
        let e = c.client.world.terrain.extent().expect("the demo has a terrain");
        let centre = Coord3D { x: (e.lo.x + e.hi.x) / 2.0, y: (e.lo.y + e.hi.y) / 2.0, z: 0.0 };
        let cam = Camera::new(&c.client.world, centre, e.hi.x - e.lo.x, (800, 600));
        (c, cam)
    }

    fn mouse(t: rts_lockstep_engine::common::message_stream::MessageType, x: i32, y: i32) -> RawMessage {
        RawMessage::mouse(t, ICoord2D { x, y })
    }

    /// *Predictions, written before the first run:* the demo's camera starts over the map's centre, south of it and
    /// above it (the demo's own pitch and height); a right button held 100 pixels right of where it went down scrolls
    /// the point to +x over ten frames and the release stops it; the arrow up scrolls it to +y; the wheel's notch lowers
    /// the height asked for by 10 and the frames settle the zoom toward it; the client's frames are 30 a second; and
    /// none of it changes the World's CRC.
    ///
    /// *Results (2026-11-10):* all held. (#117, predicted 2026-11-11: the right button's scroll asks the cursor for
    /// SCROLL, then ARROW and the kept ARROW; results: held.)
    #[test]
    fn the_camera_scrolls_turns_and_zooms_without_touching_the_world() {
        let (mut c, mut cam) = demo();
        let start = cam.view.pos;
        let (eye, target) = cam.eye_target();
        assert!(eye[1] < start.y && eye[2] > target[2], "{eye:?} {target:?}");
        let ui = c.ui.clone();
        let w = &c.client.world;
        // right button scroll
        cam.translate(&mouse(MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN, 400, 300), &ui, w, false);
        cam.translate(&mouse(MSG_RAW_MOUSE_POSITION, 500, 300), &ui, w, false);
        assert!(cam.ui_scrolling && cam.view.is_mouse_locked());
        for _ in 0..10 {
            cam.tick(&ui, w, false);
        }
        assert!(cam.view.pos.x > start.x + 1.0, "{start:?} {:?}", cam.view.pos);
        cam.translate(&mouse(MSG_RAW_MOUSE_RIGHT_BUTTON_UP, 500, 300), &ui, w, false);
        assert!(!cam.ui_scrolling && !cam.view.is_mouse_locked());
        // #117: the scroll's cursor calls: `setScrolling(TRUE)`'s SCROLL, `setScrolling(FALSE)`'s ARROW, then the
        // translator's `setCursor` of the cursor it kept (the arrow)
        {
            use rts_lockstep_engine::client::mouse::cursor;
            assert_eq!(cam.cursor_calls, [CursorCall::InGameUi(cursor::SCROLL), CursorCall::InGameUi(cursor::ARROW), CursorCall::Mouse(cursor::ARROW)]);
            cam.cursor_calls.clear();
        }
        // the arrow key
        let y0 = cam.view.pos.y;
        let k = |t, up: bool| RawMessage { t, pos: ICoord2D::default(), delta: ICoord2D::default(), arg0: i32::from(key::UP), arg1: if up { 1 } else { 2 } };
        cam.translate(&k(MSG_RAW_KEY_DOWN, false), &ui, w, false);
        for _ in 0..5 {
            cam.tick(&ui, w, false);
        }
        cam.translate(&k(MSG_RAW_KEY_UP, true), &ui, w, false);
        assert!(cam.view.pos.y > y0, "{y0} {:?}", cam.view.pos);
        // the wheel
        let h0 = cam.view.height_above_ground;
        let wheel = RawMessage { t: MSG_RAW_MOUSE_WHEEL, pos: ICoord2D::default(), delta: ICoord2D::default(), arg0: 0, arg1: 1 };
        cam.translate(&wheel, &ui, w, false);
        assert_eq!(cam.view.height_above_ground, h0 - 10.0);
        let z0 = cam.view.zoom;
        cam.tick(&ui, w, false);
        assert!(cam.view.zoom < z0);
        // the keys' rotation
        cam.keys.rotating_left = true;
        let a0 = cam.view.angle;
        cam.tick(&ui, w, false);
        assert!(cam.view.angle < a0);
        cam.keys = CameraKeys::default();
        // the clock
        assert_eq!(cam.ticks(0.1), 3);
        assert_eq!(cam.ticks(10.0), 4);
        // the reset, the radar's look, a save's view
        cam.reset(w);
        assert_eq!((cam.view.pos.x, cam.view.angle), (cam.home.x, 0.0));
        cam.look_at(Coord3D { x: cam.home.x + 20.0, y: cam.home.y, z: 0.0 }, w);
        cam.set_saved_view(0.5, cam.home, w);
        assert_eq!(cam.view.angle, 0.5);
        let (fov, near, far) = cam.projection();
        assert!(fov > 0.5 && fov < 1.2 && near == 10.0 && far >= 1200.0, "{fov} {near} {far}");
        let (p, yaw, dist) = cam.focus();
        assert!(dist > 0.0 && yaw == 0.5 && p.x == cam.home.x);
        let l = cam.listener();
        assert_eq!((l.angle, l.ground.x), (0.5, cam.home.x));
        cam.set_screen((390, 844), w);
        assert!(cam.view.portrait_scroll);
        cam.set_keyboard_scroll(2.0);
        assert_eq!(cam.globals.keyboard_scroll_factor, 2.0 * cam.globals.keyboard_default_scroll_factor);
        // the logic's CRCs with the camera scrolling between its frames are the plain game's
        let (mut plain, _) = crate::demo::demo_world();
        cam.translate(&mouse(MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN, 100, 100), &c.ui.clone(), &c.client.world, false);
        cam.translate(&mouse(MSG_RAW_MOUSE_POSITION, 150, 140), &c.ui.clone(), &c.client.world, false);
        for _ in 0..30 {
            c.client.world.logic_update().expect("a frame");
            plain.logic_update().expect("a frame");
            let ui = c.ui.clone();
            cam.tick(&ui, &c.client.world, false);
            assert_eq!(c.client.world.logic_crc(CRC_RECALC, None).expect("CRC"), plain.logic_crc(CRC_RECALC, None).expect("CRC"));
        }
    }

    /// A phone (390x844): a finger dragged up and held (the right button's scroll) moves the point; the zoom buttons'
    /// wheel notches zoom; with the input off the scroll does not start; resetModes ends one.
    ///
    /// *Predictions:* the drag 100 pixels down scrolls the point toward -y (the integer aspect would be 0 without the
    /// client's portrait scroll); the input off refuses the right button's scroll; `LookAtResetModes` clears the
    /// translator's scrolling.
    ///
    /// *Results (2026-11-10):* all held.
    #[test]
    fn a_phone_drags_and_zooms_and_the_scripts_reset_the_modes() {
        let (mut c, _) = demo();
        let e = c.client.world.terrain.extent().expect("terrain");
        let centre = Coord3D { x: (e.lo.x + e.hi.x) / 2.0, y: (e.lo.y + e.hi.y) / 2.0, z: 0.0 };
        let mut cam = Camera::new(&c.client.world, centre, e.hi.x - e.lo.x, (390, 844));
        let ui = c.ui.clone();
        let w = &c.client.world;
        let y0 = cam.view.pos.y;
        cam.translate(&mouse(MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN, 195, 400), &ui, w, false);
        cam.translate(&mouse(MSG_RAW_MOUSE_POSITION, 195, 500), &ui, w, false);
        for _ in 0..10 {
            cam.tick(&ui, w, false);
        }
        assert!(cam.view.pos.y < y0, "{y0} {:?}", cam.view.pos);
        cam.translate(&mouse(MSG_RAW_MOUSE_RIGHT_BUTTON_UP, 195, 500), &ui, w, false);
        let h0 = cam.view.height_above_ground;
        let wheel = RawMessage { t: MSG_RAW_MOUSE_WHEEL, pos: ICoord2D::default(), delta: ICoord2D::default(), arg0: 0, arg1: -3 };
        cam.translate(&wheel, &ui, w, false);
        assert!(cam.view.height_above_ground >= h0);
        cam.translate(&mouse(MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN, 195, 400), &ui, w, false);
        assert!(cam.look.is_scrolling);
        cam.reset_modes();
        assert!(!cam.look.is_scrolling);
        cam.translate(&mouse(MSG_RAW_MOUSE_RIGHT_BUTTON_UP, 195, 400), &ui, w, false);
        c.ui.set_input_enabled(false);
        let ui = c.ui.clone();
        cam.translate(&mouse(MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN, 195, 400), &ui, &c.client.world, false);
        assert!(!cam.look.is_scrolling, "setScrolling refuses with the input off");
        // the shell's input keeps the arrows from the translator
        let k = RawMessage { t: MSG_RAW_KEY_DOWN, pos: ICoord2D::default(), delta: ICoord2D::default(), arg0: i32::from(key::UP), arg1: 2 };
        cam.translate(&k, &ui, &c.client.world, true);
        assert_eq!(cam.look.scroll_dir, [false; 4]);
    }
}
