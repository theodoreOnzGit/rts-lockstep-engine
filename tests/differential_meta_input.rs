#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/MetaEvent.cpp (MetaEventTranslator's constructor and the
//     mouse part of translateGameMessage)
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/CommandXlat.cpp (the right button's raw cases,
//     3628-3657; the keyboard camera's cases, 3205-3239)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Input/Mouse.cpp (Mouse::isClick)
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (update's keyboard camera, resetCamera)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DView.cpp (resetCamera and the movements it starts)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! GitHub #131: `MetaEventTranslator`'s mouse part (`client::meta_event`), the command translator's right-button
//! bookkeeping and keyboard camera cases (`client::command_xlat`, `client::command_xlat_meta`), `Mouse::isClick`
//! (`client::mouse`), the InGameUI's keyboard camera and `resetCamera` (`client::in_game_ui`) and `W3DView::resetCamera`
//! (`client::tactical_view`) against the C++.
//!
//! # Method
//!
//! The C++ side is the upstream text cut verbatim by build.rs: the mouse part through `differential/wrap_command_meta.cpp`
//! (kind 3, with #125's stand-ins), the rest through `differential/wrap_meta_camera.cpp` (both list their cuts). Four
//! families, 20,000 random cases each (`META_INPUT_CASES`):
//!
//! - [`meta_event_mouse_matches_upstream`]: sequences of 1 to 32 raw mouse messages through one translator: the three
//!   buttons' presses, the operating system's double clicks and releases (weighted up), and the position, drags and
//!   wheel; at a point near the last one (within 0 to 12 pixels) or anywhere, sometimes past ±2^30 (the rectangle's
//!   width then wraps); random modifier bits; the drag tolerance 0, 1, 2, 4, 8 or past 2^31. Compared after each: the
//!   disposition, the click inserted (type, rectangle, modifiers), the three double-click marks and down positions.
//! - [`right_button_matches_upstream`]: sequences of 1 to 24 steps on one command translator: the right button's press
//!   and release (and a left press, a wheel, which the cases ignore) at points near each other with times 0 to 600 ms
//!   apart (sometimes wrapping past 2^32), the input turned on or off, and the right click's `isClick` test; the
//!   tolerances 0 to 8 pixels and 0, 100, 250 or 2^32-1 ms. Compared: each disposition, each `placeBuildAvailable(NULL,
//!   NULL)`, each test's answer, and the anchor, lift and times after each step.
//! - [`keyboard_camera_matches_upstream`]: sequences of 1 to 24 steps on one InGameUI: a message (the nine camera meta
//!   messages, camera tracking, the options, clear-game-data, the lower-details toggle), the input on or off, and
//!   `InGameUI::update`'s keyboard camera with a random rotate speed and starting angle. Compared: each disposition, each
//!   `resetCamera`, each `setAngle` (its bits), `zoomIn`, `zoomOut`, and the four flags after each step.
//! - [`camera_reset_matches_upstream`]: `InGameUI::resetCamera` and the next frame's `updateCameraMovements` on a view
//!   of random members (position, angle, pitch, zoom, FX pitch, ground level, camera offset, the heights, a constraint:
//!   half the time with the movement on one the position is outside of, else one holding it), the global data's camera
//!   pitch and yaw, and `m_disableCameraMovement` 1 time in 8, on a
//!   ground of steps and a slope. Compared (bits): the position, angle, zoom, FX pitch, ground level, camera offset,
//!   height asked for and constraint after, and that the four movements have ended (the port has no such flags).
//!
//! *Predictions, before the first run* (`issue131_progress.md`): 0 mismatches in the four; every plant 9190-9210 caught.
//!
//! # Results (2026-11-11, `META_INPUT_CASES=20000`, the default)
//!
//! - Mouse part: 20,000 sequences, **0 mismatches**; 109,935 clicks, 22,638 of them double, 20,643 points.
//! - Right button: 20,000 sequences, **0 mismatches**; 4,525 placements cancelled, 50,161 click tests of which 10,909
//!   were clicks.
//! - Keyboard camera: 20,000 sequences, **0 mismatches** on the second run; 18,487 sequences turned the view, 18,837
//!   zoomed it, 9,614 asked for a reset. **The first run had 3,707**: the harness, not the translation: the C++ step that
//!   turns the input off assigned `m_inputEnabled` where the Rust called `setInputEnabled`, which clears the four flags;
//!   the wrapper now compiles upstream's `setInputEnabled` too.
//! - Camera reset: 20,000 cases, **0 mismatches**; 2,487 with camera movement disabled. The first generator's
//!   constraint always held the position and its ground never rose past 120, so plants 9209 and 9210 went uncaught;
//!   the generator now puts the position outside the constraint half the time (with the movement on) and the slope
//!   past 120 (the counts below are that run's: 0 mismatches before and after).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: **21 of 21 caught** (2026-11-11).
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 9190 | a press does not clear its button's double click | 9,475 mouse sequences |
//! | 9191 | the middle button's index is the right one's | 16,708 |
//! | 9192 | the drag tolerance compared with `<=` | 668 |
//! | 9193 | a release keeps the double click mark | 12,249 |
//! | 9194 | the click without the release's modifiers | 16,879 |
//! | 9195 | the point test on the width only | 1,310 |
//! | 9196 | every right release cancels the placement | 16,394 right-button sequences |
//! | 9197 | `isClick` compares with `>=` | 1,682 |
//! | 9198 | `isClick` without the time | 4,694 |
//! | 9199 | the right press's time not kept | 16,931 |
//! | 9200 | the right click's `isClick` always true (the code before #131) | 14,301 |
//! | 9201 | `END_CAMERA_ROTATE_LEFT` clears the right flag | 2,949 keyboard-camera sequences |
//! | 9202 | both rotations turn left | 1,603 |
//! | 9203 | both zooms zoom in | 1,579 |
//! | 9204 | the reset at the origin, not the view's position | 17,513 resets |
//! | 9205 | `setInputEnabled` leaves the camera flags | 3,707 (and 1,292 in `differential_input_gate`) |
//! | 9206 | the reset keeps the angle | 17,513 |
//! | 9207 | the reset's zoom from the height asked for, not the maximum | 17,513 |
//! | 9208 | the reset keeps the FX pitch | 17,513 |
//! | 9209 | the constraint not widened | 8,677 |
//! | 9210 | the reset's ground level capped at 120 (`initHeightForMap`'s) | 5,208 |

mod support;

use rts_lockstep_engine::client::command_xlat::{input_disabled_destroys, CommandTranslator, MouseClickHost};
use rts_lockstep_engine::client::command_xlat_meta::CameraMetaHost;
use rts_lockstep_engine::client::in_game_ui::{DrawableId, InGameUiSelection, KeyboardCameraView};
use rts_lockstep_engine::client::in_game_ui_cursor::UiMouseCall;
use rts_lockstep_engine::client::meta_event::MetaEventTranslator;
use rts_lockstep_engine::client::selection_info::SelectionInfoHost;
use rts_lockstep_engine::client::selection_xlat::Disposition;
use rts_lockstep_engine::client::tactical_view::{TacticalView, ViewHost};
use rts_lockstep_engine::common::base_type::{Coord3D, ICoord2D, Region3D};
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::global_data::GlobalData;
use rts_lockstep_engine::common::kind_of::KindOfType;
use rts_lockstep_engine::common::message_stream::MessageType;
use rts_lockstep_engine::common::message_types::*;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

/// (CPP_LOCK: the two wrappers' state is global and the tests run on several threads)
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn cpp_meta(kind: i32, input: &[i32]) -> Vec<i32> {
    let _held = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = vec![0i32; 1 << 14];
    let n = unsafe { cpp::ref_command_meta_case(kind, input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!((n as usize) <= out.len());
    out.truncate(n as usize);
    out
}

fn cpp_camera(kind: i32, input: &[i32]) -> Vec<i32> {
    let _held = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = vec![0i32; 1 << 14];
    let n = unsafe { cpp::ref_meta_camera_case(kind, input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!((n as usize) <= out.len());
    out.truncate(n as usize);
    out
}

fn cases() -> u64 {
    std::env::var("META_INPUT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish(c: Check) {
    if planted() {
        c.report();
    } else {
        c.finish();
    }
}

fn disp(d: Disposition) -> i32 {
    match d {
        Disposition::Keep => 0,
        Disposition::Destroy => 1,
    }
}

/// A point near `last` (0 to 12 pixels off) or anywhere, sometimes far out.
fn point(g: &mut Gen, last: ICoord2D) -> ICoord2D {
    match g.below(8) {
        0 => ICoord2D { x: g.below(2001) as i32 - 1000, y: g.below(2001) as i32 - 1000 },
        1 => ICoord2D { x: g.i32() >> g.below(3), y: g.i32() >> g.below(3) },
        _ => ICoord2D { x: last.x.wrapping_add(g.below(13) as i32 - 6), y: last.y.wrapping_add(g.below(13) as i32 - 6) },
    }
}

/// `MetaEventTranslator`'s mouse part on random sequences (see the module docs).
#[test]
fn meta_event_mouse_matches_upstream() {
    const TYPES: [MessageType; 15] = [
        MSG_RAW_MOUSE_LEFT_BUTTON_DOWN,
        MSG_RAW_MOUSE_LEFT_DOUBLE_CLICK,
        MSG_RAW_MOUSE_LEFT_BUTTON_UP,
        MSG_RAW_MOUSE_MIDDLE_BUTTON_DOWN,
        MSG_RAW_MOUSE_MIDDLE_DOUBLE_CLICK,
        MSG_RAW_MOUSE_MIDDLE_BUTTON_UP,
        MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN,
        MSG_RAW_MOUSE_RIGHT_DOUBLE_CLICK,
        MSG_RAW_MOUSE_RIGHT_BUTTON_UP,
        MSG_RAW_MOUSE_LEFT_BUTTON_UP,
        MSG_RAW_MOUSE_RIGHT_BUTTON_UP,
        MSG_RAW_MOUSE_POSITION,
        MSG_RAW_MOUSE_LEFT_DRAG,
        MSG_RAW_MOUSE_WHEEL,
        MSG_RAW_MOUSE_LEFT_CLICK,
    ];
    let mut g = Gen::new(0x131_0001);
    let mut c = Check::new("MetaEventTranslator mouse part");
    let (mut clicks, mut doubles, mut points) = (0u64, 0u64, 0u64);
    for i in 0..cases() {
        let tol: u32 = [0, 1, 2, 4, 8, 0x8000_0001][g.below(6) as usize];
        let mut input = vec![tol as i32];
        let mut x = MetaEventTranslator::default();
        let mut rust = Vec::new();
        let mut last = ICoord2D { x: g.below(800) as i32, y: g.below(600) as i32 };
        for _ in 0..1 + g.below(32) {
            let t = TYPES[g.below(15) as usize];
            let p = point(&mut g, last);
            last = p;
            let mods = if g.below(2) == 0 { 0 } else { g.i32() };
            input.extend([t.0 as i32, p.x, p.y, mods]);
            let click = x.translate_mouse(t, p, mods, tol);
            rust.push(0);
            if let Some(k) = click {
                clicks += 1;
                doubles += u64::from(k.t == MSG_MOUSE_LEFT_DOUBLE_CLICK || k.t == MSG_MOUSE_MIDDLE_DOUBLE_CLICK || k.t == MSG_MOUSE_RIGHT_DOUBLE_CLICK);
                points += u64::from(k.is_point());
                rust.extend([k.t.0 as i32, k.region.lo.x, k.region.lo.y, k.region.hi.x, k.region.hi.y, k.modifiers]);
            }
            rust.push(-1);
            rust.extend(x.next_up_should_create_double_click.iter().map(|&b| i32::from(b)));
            for d in x.mouse_down_position {
                rust.extend([d.x, d.y]);
            }
            rust.push(-2);
        }
        c.eq(rust, cpp_meta(3, &input), || format!("sequence {i} {input:?}"));
    }
    println!("mouse sequences: {clicks} clicks, {doubles} of them double, {points} points");
    finish(c);
}

/// The right button's host: `TheMouse`'s tolerances, and nothing selected.
struct RightHost {
    tol: (u32, u32),
}

impl MouseClickHost for RightHost {
    fn mouse_is_click(&self, anchor: &ICoord2D, dest: &ICoord2D, previous: u32, current: u32) -> bool {
        rts_lockstep_engine::client::mouse::is_click(anchor, dest, previous, current, self.tol.0, self.tol.1)
    }
}

impl SelectionInfoHost for RightHost {
    fn drawable_object(&self, _draw: DrawableId) -> Option<ObjectId> {
        None
    }
    fn is_locally_controlled(&self, _obj: ObjectId) -> bool {
        false
    }
    fn is_kind_of(&self, _obj: ObjectId, _k: KindOfType) -> bool {
        false
    }
    fn relationship_to_local_player(&self, _obj: ObjectId) -> Relationship {
        Relationship::Neutral
    }
    fn can_local_player_garrison(&mut self, _obj: ObjectId) -> bool {
        false
    }
    fn force_fire_or_move(&self) -> bool {
        false
    }
    fn prefer_selection(&self) -> bool {
        false
    }
    fn use_alternate_mouse(&self) -> bool {
        false
    }
}

/// The command translator's right-button bookkeeping and `Mouse::isClick` on random sequences (see the module docs).
#[test]
fn right_button_matches_upstream() {
    let mut g = Gen::new(0x131_0002);
    let mut c = Check::new("CommandTranslator right button");
    let (mut cancels, mut tests, mut clicks) = (0u64, 0u64, 0u64);
    for i in 0..cases() {
        let tol = (g.below(9), [0u32, 100, 250, u32::MAX][g.below(4) as usize]);
        let mut input = vec![tol.0 as i32, tol.1 as i32];
        let mut h = RightHost { tol };
        let mut x = CommandTranslator::default();
        let mut ui = InGameUiSelection::new();
        let mut rust = Vec::new();
        let mut last = ICoord2D { x: g.below(800) as i32, y: g.below(600) as i32 };
        let mut time: u32 = if g.below(8) == 0 { u32::MAX - g.below(300) } else { g.below(100_000) };
        for _ in 0..1 + g.below(24) {
            match g.below(10) {
                0..=6 => {
                    let t = match g.below(10) {
                        0 => MSG_RAW_MOUSE_LEFT_BUTTON_DOWN,
                        1 => MSG_RAW_MOUSE_WHEEL,
                        2..=5 => MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN,
                        _ => MSG_RAW_MOUSE_RIGHT_BUTTON_UP,
                    };
                    let p = point(&mut g, last);
                    last = p;
                    time = time.wrapping_add(g.below(301) * u32::from(g.below(2) + 1));
                    input.extend([0, t.0 as i32, p.x, p.y, time as i32]);
                    let destroyed = if t == MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN {
                        x.right_button_down(p, time, &ui)
                    } else if t == MSG_RAW_MOUSE_RIGHT_BUTTON_UP {
                        x.right_button_up(p, time, &mut ui, &mut h)
                    } else {
                        input_disabled_destroys(t, ui.input_enabled())
                    };
                    let placed = ui.take_mouse_calls().into_iter().filter(|m| *m == UiMouseCall::ReleaseCapture).count();
                    cancels += placed as u64;
                    rust.extend(std::iter::repeat_n(4000, placed));
                    rust.push(i32::from(destroyed));
                }
                7 => {
                    let on = g.below(3) != 0;
                    input.extend([1, i32::from(on), 0, 0, 0]);
                    ui.set_input_enabled(on);
                    let _ = ui.take_mouse_calls();
                }
                _ => {
                    input.extend([2, 0, 0, 0, 0]);
                    let r = x.right_is_click(&h);
                    tests += 1;
                    clicks += u64::from(r);
                    rust.extend([5000, i32::from(r)]);
                }
            }
            rust.extend([-1, x.mouse_right_drag_anchor.x, x.mouse_right_drag_anchor.y, x.mouse_right_drag_lift.x, x.mouse_right_drag_lift.y, x.mouse_right_down as i32, x.mouse_right_up as i32]);
        }
        c.eq(rust, cpp_camera(0, &input), || format!("sequence {i} {input:?}"));
    }
    println!("right button sequences: {cancels} placements cancelled, {tests} click tests of which {clicks} clicks");
    finish(c);
}

/// `TheTacticalView` as a recorder of the keyboard camera's calls, in the C++ stand-in's words.
struct ViewRecorder<'a> {
    angle: f32,
    out: &'a mut Vec<i32>,
}

impl KeyboardCameraView for ViewRecorder<'_> {
    fn get_angle(&self) -> f32 {
        self.angle
    }
    fn set_angle(&mut self, angle: f32) {
        self.out.extend([5001, angle.to_bits() as i32]);
        self.angle = angle;
    }
    fn zoom_in(&mut self) {
        self.out.push(5002);
    }
    fn zoom_out(&mut self) {
        self.out.push(5003);
    }
}

struct ResetRecorder<'a> {
    out: &'a mut Vec<i32>,
}

impl CameraMetaHost for ResetRecorder<'_> {
    fn reset_camera(&mut self) {
        self.out.push(5004);
    }
}

/// The keyboard camera's meta cases and `InGameUI::update`'s keyboard camera on random sequences (see the module docs).
#[test]
fn keyboard_camera_matches_upstream() {
    const TYPES: [MessageType; 13] = [
        MSG_META_BEGIN_CAMERA_ROTATE_LEFT,
        MSG_META_END_CAMERA_ROTATE_LEFT,
        MSG_META_BEGIN_CAMERA_ROTATE_RIGHT,
        MSG_META_END_CAMERA_ROTATE_RIGHT,
        MSG_META_BEGIN_CAMERA_ZOOM_IN,
        MSG_META_END_CAMERA_ZOOM_IN,
        MSG_META_BEGIN_CAMERA_ZOOM_OUT,
        MSG_META_END_CAMERA_ZOOM_OUT,
        MSG_META_CAMERA_RESET,
        MSG_META_TOGGLE_CAMERA_TRACKING_DRAWABLE,
        MSG_META_OPTIONS,
        MSG_CLEAR_GAME_DATA,
        MSG_META_TOGGLE_LOWER_DETAILS,
    ];
    let mut g = Gen::new(0x131_0003);
    let mut c = Check::new("keyboard camera");
    let (mut turns, mut zooms, mut resets) = (0u64, 0u64, 0u64);
    for i in 0..cases() {
        let speed = g.uniform(0.0, 0.2);
        let mut angle = g.uniform(-4.0, 4.0);
        let mut input = vec![speed.to_bits() as i32, angle.to_bits() as i32];
        let mut ui = InGameUiSelection::new();
        let mut rust: Vec<i32> = Vec::new();
        for _ in 0..1 + g.below(24) {
            match g.below(10) {
                0..=5 => {
                    let t = TYPES[g.below(13) as usize];
                    input.extend([0, t.0 as i32]);
                    let d = {
                        let mut h = ResetRecorder { out: &mut rust };
                        CommandTranslator::camera_meta(t, &mut ui, &mut h)
                    };
                    resets += u64::from(rust.last() == Some(&5004));
                    let d = d.unwrap_or_else(|| if input_disabled_destroys(t, ui.input_enabled()) { Disposition::Destroy } else { Disposition::Keep });
                    rust.push(disp(d));
                }
                6 => {
                    let on = g.below(3) != 0;
                    input.extend([1, i32::from(on)]);
                    ui.set_input_enabled(on);
                }
                _ => {
                    input.extend([2, 0]);
                    let before = rust.len();
                    let mut v = ViewRecorder { angle, out: &mut rust };
                    ui.update_keyboard_camera(&mut v, speed);
                    angle = v.angle;
                    turns += u64::from(rust[before..].contains(&5001));
                    zooms += u64::from(rust[before..].iter().any(|&w| w == 5002 || w == 5003));
                }
            }
            rust.extend([
                -1,
                i32::from(ui.is_camera_rotating_left()),
                i32::from(ui.is_camera_rotating_right()),
                i32::from(ui.is_camera_zooming_in()),
                i32::from(ui.is_camera_zooming_out()),
            ]);
        }
        c.eq(rust, cpp_camera(1, &input), || format!("sequence {i} {input:?}"));
    }
    println!("keyboard camera sequences: {turns} updates turned, {zooms} zoomed, {resets} resets asked");
    finish(c);
}

/// The reset's view host: a ground of steps and a slope, the case's global data.
struct ResetGround {
    g: GlobalData,
}

impl ViewHost for ResetGround {
    fn global(&self) -> &GlobalData {
        &self.g
    }
    fn ground_height(&self, x: f32, y: f32) -> f32 {
        ((x / 64.0f32) as i32 % 5) as f32 * 7.5f32 + y / 128.0f32
    }
    fn extent(&self) -> Option<Region3D> {
        None
    }
    fn game_paused(&self) -> bool {
        false
    }
}

/// `InGameUI::resetCamera` and the frame after it (see the module docs).
#[test]
fn camera_reset_matches_upstream() {
    let mut g = Gen::new(0x131_0004);
    let mut c = Check::new("InGameUI::resetCamera and W3DView");
    let mut disabled = 0u64;
    for i in 0..cases() {
        let mut gd = GlobalData::new();
        gd.camera_pitch = g.uniform(20.0, 80.0);
        gd.camera_yaw = g.uniform(-30.0, 30.0);
        gd.disable_camera_movement = g.below(8) == 0;
        disabled += u64::from(gd.disable_camera_movement);
        let h = ResetGround { g: gd };
        let mut v = TacticalView::new(&h.g);
        // (y up to 20,000: the slope's ground then rises past 120, `initHeightForMap`'s cap, which the reset does not apply)
        v.pos = Coord3D { x: g.uniform(-500.0, 3000.0), y: g.uniform(-500.0, 20000.0), z: if g.below(2) == 0 { 0.0 } else { g.uniform(-5.0, 50.0) } };
        v.angle = g.uniform(-3.5, 3.5);
        v.pitch_angle = g.uniform(-0.6, 0.6);
        v.zoom = g.uniform(0.2, 2.0);
        v.fx_pitch = g.uniform(0.1, 3.0);
        v.ground_level = g.uniform(0.0, 150.0);
        v.camera_offset = Coord3D { x: g.uniform(-50.0, 50.0), y: g.uniform(-400.0, -50.0), z: g.uniform(50.0, 500.0) };
        v.max_height_above_ground = g.uniform(100.0, 600.0);
        v.height_above_ground = g.uniform(50.0, 600.0);
        // (half the time with the movement on, a constraint the position is outside, which the reset widens; else one
        // holding it: the port places the camera at once, where the C++ frame's `setCameraTransform` is not run)
        let outside = !h.g.disable_camera_movement && g.below(2) == 0;
        let (dx, dy) = if outside { (g.uniform(-600.0, 600.0), g.uniform(-600.0, 600.0)) } else { (0.0, 0.0) };
        v.camera_constraint.lo.x = v.pos.x + dx - g.uniform(0.0, 300.0);
        v.camera_constraint.lo.y = v.pos.y + dy - g.uniform(0.0, 300.0);
        v.camera_constraint.hi.x = v.camera_constraint.lo.x + g.uniform(0.0, 300.0);
        v.camera_constraint.hi.y = v.camera_constraint.lo.y + g.uniform(0.0, 300.0);
        if !outside {
            let r = &mut v.camera_constraint;
            if r.lo.x > v.pos.x {
                r.lo.x = v.pos.x;
            }
            if r.lo.y > v.pos.y {
                r.lo.y = v.pos.y;
            }
            if r.hi.x < v.pos.x {
                r.hi.x = v.pos.x;
            }
            if r.hi.y < v.pos.y {
                r.hi.y = v.pos.y;
            }
        }
        v.camera_constraint_valid = true;
        let b = |f: f32| f.to_bits() as i32;
        let input = vec![
            b(v.pos.x),
            b(v.pos.y),
            b(v.pos.z),
            b(v.angle),
            b(v.pitch_angle),
            b(v.zoom),
            b(v.fx_pitch),
            b(v.ground_level),
            b(v.camera_offset.x),
            b(v.camera_offset.y),
            b(v.camera_offset.z),
            b(v.max_height_above_ground),
            b(v.height_above_ground),
            b(v.camera_constraint.lo.x),
            b(v.camera_constraint.lo.y),
            b(v.camera_constraint.hi.x),
            b(v.camera_constraint.hi.y),
            b(h.g.camera_pitch),
            b(h.g.camera_yaw),
            i32::from(h.g.disable_camera_movement),
        ];
        rts_lockstep_engine::client::in_game_ui::reset_camera(&mut v, &h);
        let rust = vec![
            b(v.pos.x),
            b(v.pos.y),
            b(v.pos.z),
            b(v.angle),
            b(v.zoom),
            b(v.fx_pitch),
            b(v.ground_level),
            b(v.camera_offset.x),
            b(v.camera_offset.y),
            b(v.camera_offset.z),
            b(v.height_above_ground),
            b(v.camera_constraint.lo.x),
            b(v.camera_constraint.lo.y),
            b(v.camera_constraint.hi.x),
            b(v.camera_constraint.hi.y),
            // (the four movements have ended after the frame)
            0,
            0,
            0,
            0,
        ];
        c.eq(rust, cpp_camera(2, &input), || format!("case {i} {input:?}"));
    }
    println!("camera resets: {disabled} with camera movement disabled");
    finish(c);
}
