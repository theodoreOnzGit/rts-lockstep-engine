// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/LookAtXlat.cpp (the whole file, as the release build
//     compiles it)
//   GeneralsMD/Code/GameEngine/Include/GameClient/LookAtXlat.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `LookAtTranslator` (GitHub #119), the message stream's camera translator (priority 60): the right button, arrow key
//! and screen edge scrolls, the middle button's rotation and its click back to the default view, the wheel's zoom, and
//! the eight view bookmarks.
//!
//! # As upstream reads it
//!
//! A scroll is one of three modes (`m_scrollType`): the right button held (the view moves by the scroll speed factors
//! times the pointer's distance from where the button went down, plus a minimum along that direction), an arrow key
//! held (100 times the keyboard scroll factor per frame along each held direction), or the pointer within 3 pixels of
//! the screen's edge in full screen. `setScrolling` refuses with the InGameUI's input off; it saves the cursor, locks the
//! tactical view's mouse and tells the InGameUI. Each `MSG_FRAME_TICK` scrolls the view by the mode's offset (and stops a
//! scroll the InGameUI was told to stop, a script's), and in a single player or skirmish game with the global data's
//! replay camera on, appends `MSG_SET_REPLAY_CAMERA` with the view's location. A mouse move with the input off stops
//! any scroll. The middle button turns the view by 0.01 radian per pixel the pointer moves; pressed and released within
//! 5 client frames and 5 pixels it resets the angle, the pitch and the zoom. The wheel zooms one step per notch.
//! `MSG_META_SAVE_VIEWn` keeps the view's location in slot n and shows "bookmark set"; `MSG_META_VIEW_VIEWn` goes back
//! to it. Both are destroyed; everything else is kept.
//!
//! # Upstream quirks that are kept
//!
//! - **The wheel falls through into `MSG_META_OPTIONS`** (no `break`): every wheel message stops the scroll, scrolling
//!   or not, and sets the cursor back to the one saved when the last scroll started.
//! - `stopScrolling` runs when nothing scrolls too (the wheel, `MSG_META_OPTIONS`), so it unlocks the view's mouse and
//!   sets the saved cursor (the arrow before any scroll) each time.
//! - The middle button's click test takes the absolute value of the x movement only, and compares both movements with
//!   an unsigned 5 (`const UnsignedInt PIXEL_OFFSET`), so a move up by even one pixel counts as moved (and a move of
//!   `INT_MIN` in x, whose negation overflows, too).
//! - The edge tests compare a signed position with the display's unsigned size less 3: a negative position is past the
//!   bottom and right edges too (so a scroll at a negative y moves both up and down, which nearly cancel), and on a
//!   display smaller than 3 pixels the size wraps.
//! - The arrow keys' directions (`scrollDir`) and the saved cursor (`prevCursor`) are file statics upstream; they are
//!   fields here (there is one translator, as upstream asserts).
//! - The key and its state are truncated to bytes (`UnsignedByte`).
//!
//! # Where this differs from upstream
//!
//! - The debug build's cases are not translated (`_DEBUG`/`_INTERNAL`: the pitch and field of view adjustments, the
//!   shroud cheats, the plane camera lock; `_ALLOW_DEBUG_CHEATS_IN_RELEASE`: `MSG_CHEAT_DESHROUD`). In the release
//!   build `m_isPitching` and `m_isChangingFOV` are never set, so the mouse move's pitch branch never runs; it is kept.
//! - The client application has no message stream: its input glue (`client/src/app.rs`) gives each raw input and the
//!   frame tick to [`LookAtTranslator::translate_game_message`] as a [`RawMessage`].
//! - `TheInGameUI`, `TheMouse`, `TheShell`, `TheStatsCollector`, `TheGameLogic`, `TheGameClient`, `TheDisplay`,
//!   `TheGlobalData`, `TheMessageStream` and `TheTacticalView` are asked through [`LookAtHost`].
//!
//! # Representation
//!
//! The translator is [`LookAtTranslator`] (its members, and the two file statics); the scroll modes are the `SCROLL_*`
//! constants. The view's location is the view's [`ViewLocation`].

use crate::client::selection_xlat::Disposition;
use crate::client::tactical_view::ViewLocation;
use crate::client::window_manager::key;
use crate::client::window_xlat::RawMessage;
use crate::common::base_type::{sqr, Coord2D, ICoord2D};
use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::message_types::*;
use crate::logic::modules::plant;

/// `MAX_VIEW_LOCS`.
pub const MAX_VIEW_LOCS: usize = 8;
/// `SCROLL_NONE`.
pub const SCROLL_NONE: i32 = 0;
/// `SCROLL_RMB`.
pub const SCROLL_RMB: i32 = 1;
/// `SCROLL_KEY`.
pub const SCROLL_KEY: i32 = 2;
/// `SCROLL_SCREENEDGE`.
pub const SCROLL_SCREENEDGE: i32 = 3;
/// `DIR_UP`, `DIR_DOWN`, `DIR_LEFT`, `DIR_RIGHT`.
const DIR_UP: usize = 0;
const DIR_DOWN: usize = 1;
const DIR_LEFT: usize = 2;
const DIR_RIGHT: usize = 3;
/// `SCROLL_AMT` (a global `Int` upstream, never written).
pub const SCROLL_AMT: i32 = 100;
/// `edgeScrollSize`.
const EDGE_SCROLL_SIZE: i32 = 3;
/// `Mouse::ARROW`.
pub const MOUSE_ARROW: i32 = 2;

/// The global data the translator reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LookAtGlobals {
    /// `m_windowed`.
    pub windowed: bool,
    /// `m_horizontalScrollSpeedFactor`.
    pub horizontal_scroll_speed_factor: f32,
    /// `m_verticalScrollSpeedFactor`.
    pub vertical_scroll_speed_factor: f32,
    /// `m_keyboardScrollFactor`.
    pub keyboard_scroll_factor: f32,
    /// `m_saveCameraInReplay`.
    pub save_camera_in_replay: bool,
}

/// What `LookAtTranslator` asks of the engine.
pub trait LookAtHost {
    /// `TheInGameUI->getInputEnabled()`.
    fn ui_input_enabled(&self) -> bool;
    /// `TheInGameUI->isSelecting()`.
    fn ui_is_selecting(&self) -> bool;
    /// `TheInGameUI->setScrolling(b)`.
    fn ui_set_scrolling(&mut self, b: bool);
    /// `TheInGameUI->isScrolling()`.
    fn ui_is_scrolling(&self) -> bool;
    /// `TheInGameUI->setScrollAmount(amt)`.
    fn ui_set_scroll_amount(&mut self, amt: Coord2D);
    /// `TheInGameUI->shouldMoveRMBScrollAnchor()`.
    fn ui_should_move_rmb_scroll_anchor(&self) -> bool;
    /// `TheInGameUI->message(format(TheGameText->fetch("GUI:BookmarkXSet"), slot))`.
    fn ui_message_bookmark_set(&mut self, slot: i32);
    /// `TheMouse->getMouseCursor()`.
    fn mouse_cursor(&self) -> i32;
    /// `TheMouse->setCursor(cursor)`.
    fn mouse_set_cursor(&mut self, cursor: i32);
    /// `TheShell && TheShell->isShellActive()`.
    fn shell_active(&self) -> bool;
    /// `TheStatsCollector != NULL`.
    fn stats_collector(&self) -> bool;
    /// `TheStatsCollector->startScrollTime()`.
    fn stats_start_scroll_time(&mut self);
    /// `TheStatsCollector->endScrollTime()`.
    fn stats_end_scroll_time(&mut self);
    /// `TheGameLogic->getFrame()`.
    fn logic_frame(&self) -> u32;
    /// `TheGameClient->getFrame()`.
    fn client_frame(&self) -> u32;
    /// `TheDisplay->getWidth()`, `getHeight()`.
    fn display_width(&self) -> u32;
    fn display_height(&self) -> u32;
    /// `TheGlobalData`'s fields.
    fn globals(&self) -> LookAtGlobals;
    /// `TheGameLogic->isInSinglePlayerGame()`, `isInSkirmishGame()`.
    fn in_single_player_game(&self) -> bool;
    fn in_skirmish_game(&self) -> bool;
    /// `TheMessageStream->appendMessage(MSG_SET_REPLAY_CAMERA)` with the location's position, angle, pitch and zoom,
    /// the cursor and the pointer's position.
    fn append_replay_camera(&mut self, loc: &ViewLocation, cursor: i32, pos: ICoord2D);
    /// `TheTacticalView->setMouseLock(b)`.
    fn view_set_mouse_lock(&mut self, b: bool);
    /// `TheTacticalView->setAngleAndPitchToDefault()`.
    fn view_set_angle_and_pitch_to_default(&mut self);
    /// `TheTacticalView->setZoomToDefault()`.
    fn view_set_zoom_to_default(&mut self);
    /// `TheTacticalView->getAngle()`, `setAngle(a)`.
    fn view_angle(&self) -> f32;
    fn view_set_angle(&mut self, a: f32);
    /// `TheTacticalView->getPitch()`, `setPitch(a)`.
    fn view_pitch(&self) -> f32;
    fn view_set_pitch(&mut self, a: f32);
    /// `TheTacticalView->zoomIn()`, `zoomOut()`.
    fn view_zoom_in(&mut self);
    fn view_zoom_out(&mut self);
    /// `TheTacticalView->scrollBy(&offset)`.
    fn view_scroll_by(&mut self, offset: &Coord2D);
    /// `TheTacticalView->getLocation(&loc)`.
    fn view_get_location(&mut self) -> ViewLocation;
    /// `TheTacticalView->setLocation(&loc)`.
    fn view_set_location(&mut self, loc: &ViewLocation);
}

/// `class LookAtTranslator`.
#[derive(Clone, Debug, PartialEq)]
pub struct LookAtTranslator {
    /// `m_anchor`.
    pub anchor: ICoord2D,
    /// `m_originalAnchor`.
    pub original_anchor: ICoord2D,
    /// `m_currentPos`.
    pub current_pos: ICoord2D,
    /// `m_isScrolling`.
    pub is_scrolling: bool,
    /// `m_isRotating`.
    pub is_rotating: bool,
    /// `m_isPitching`.
    pub is_pitching: bool,
    /// `m_isChangingFOV`.
    pub is_changing_fov: bool,
    /// `m_timestamp` (the client frame the middle button went down).
    pub timestamp: u32,
    /// `m_viewLocation[MAX_VIEW_LOCS]`.
    pub view_location: [ViewLocation; MAX_VIEW_LOCS],
    /// `m_scrollType`.
    pub scroll_type: i32,
    /// `m_lastMouseMoveFrame`.
    pub last_mouse_move_frame: u32,
    /// The file static `scrollDir[4]`.
    pub scroll_dir: [bool; 4],
    /// The file static `prevCursor` (`Mouse::ARROW` at the start).
    pub prev_cursor: i32,
}

impl Default for LookAtTranslator {
    fn default() -> Self {
        Self::new()
    }
}

/// `a < b` for an `Int` and an `UnsignedInt` (`a` converted to unsigned).
fn lt_unsigned(a: i32, b: u32) -> bool {
    (a as u32) < b
}

impl LookAtTranslator {
    /// `LookAtTranslator()`: nothing scrolling, every position 0.
    pub fn new() -> Self {
        LookAtTranslator {
            anchor: ICoord2D::default(),
            original_anchor: ICoord2D::default(),
            current_pos: ICoord2D::default(),
            is_scrolling: false,
            is_rotating: false,
            is_pitching: false,
            is_changing_fov: false,
            timestamp: 0,
            view_location: [ViewLocation::default(); MAX_VIEW_LOCS],
            scroll_type: SCROLL_NONE,
            last_mouse_move_frame: 0,
            scroll_dir: [false; 4],
            prev_cursor: MOUSE_ARROW,
        }
    }

    /// `setScrolling(x)`: with the input on, starts a scroll of mode `x`.
    fn set_scrolling(&mut self, x: i32, h: &mut impl LookAtHost) {
        if !h.ui_input_enabled() && !plant(9041) {
            return;
        }
        self.prev_cursor = h.mouse_cursor();
        self.is_scrolling = true;
        h.ui_set_scrolling(true);
        h.view_set_mouse_lock(true);
        self.scroll_type = x;
        if h.stats_collector() {
            h.stats_start_scroll_time();
        }
    }

    /// `stopScrolling()`.
    fn stop_scrolling(&mut self, h: &mut impl LookAtHost) {
        self.is_scrolling = false;
        h.ui_set_scrolling(false);
        h.view_set_mouse_lock(false);
        h.mouse_set_cursor(self.prev_cursor);
        self.scroll_type = SCROLL_NONE;
        if h.stats_collector() {
            h.stats_end_scroll_time();
        }
    }

    /// `getRMBScrollAnchor()`: the anchor while a right button scroll is under way.
    pub fn get_rmb_scroll_anchor(&self) -> Option<ICoord2D> {
        if self.is_scrolling && self.scroll_type == SCROLL_RMB {
            return Some(self.anchor);
        }
        None
    }

    /// `hasMouseMovedRecently()`: within a second of logic frames (the frame reset when it is ahead of the logic's).
    pub fn has_mouse_moved_recently(&mut self, logic_frame: u32) -> bool {
        if self.last_mouse_move_frame > logic_frame {
            self.last_mouse_move_frame = 0;
        }
        if self.last_mouse_move_frame.wrapping_add(LOGICFRAMES_PER_SECOND as u32) < logic_frame {
            return false;
        }
        true
    }

    /// `setCurrentPos(pos)`.
    pub fn set_current_pos(&mut self, pos: ICoord2D) {
        self.current_pos = pos;
    }

    /// `resetModes()`: no scroll, rotation, pitch or field of view mode (the InGameUI and the view are not told).
    pub fn reset_modes(&mut self) {
        self.is_scrolling = false;
        if !plant(9042) {
            self.is_rotating = false;
        }
        self.is_pitching = false;
        self.is_changing_fov = false;
    }

    /// `translateGameMessage(msg)`.
    pub fn translate_game_message(&mut self, msg: &RawMessage, h: &mut impl LookAtHost) -> Disposition {
        let mut disp = Disposition::Keep;
        let t = msg.t;
        if t == MSG_RAW_KEY_DOWN || t == MSG_RAW_KEY_UP {
            let k = msg.arg0 as u8;
            let state = msg.arg1 as u8;
            let is_pressed = (u32::from(state) & key::STATE_UP) == 0;
            if h.shell_active() {
                return disp;
            }
            match k {
                key::UP => self.scroll_dir[DIR_UP] = is_pressed,
                key::DOWN => self.scroll_dir[DIR_DOWN] = is_pressed,
                key::LEFT => self.scroll_dir[DIR_LEFT] = is_pressed,
                key::RIGHT => self.scroll_dir[DIR_RIGHT] = is_pressed,
                _ => {}
            }
            if h.ui_is_selecting() || (self.is_scrolling && self.scroll_type != SCROLL_KEY) {
                return disp;
            }
            let num_dirs = self.scroll_dir.iter().filter(|&&d| d).count();
            if num_dirs != 0 && !self.is_scrolling {
                self.set_scrolling(SCROLL_KEY, h);
            } else if num_dirs == 0 && self.is_scrolling {
                self.stop_scrolling(h);
            }
        } else if t == MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN {
            self.last_mouse_move_frame = h.logic_frame();
            self.anchor = msg.pos;
            self.current_pos = msg.pos;
            if !h.ui_is_selecting() && !self.is_scrolling {
                self.set_scrolling(SCROLL_RMB, h);
            }
        } else if t == MSG_RAW_MOUSE_RIGHT_BUTTON_UP {
            self.last_mouse_move_frame = h.logic_frame();
            if self.scroll_type == SCROLL_RMB {
                self.stop_scrolling(h);
            }
        } else if t == MSG_RAW_MOUSE_MIDDLE_BUTTON_DOWN {
            self.last_mouse_move_frame = h.logic_frame();
            self.is_rotating = true;
            self.anchor = msg.pos;
            self.original_anchor = msg.pos;
            self.current_pos = msg.pos;
            self.timestamp = h.client_frame();
        } else if t == MSG_RAW_MOUSE_MIDDLE_BUTTON_UP {
            self.last_mouse_move_frame = h.logic_frame();
            const CLICK_DURATION: u32 = 5;
            const PIXEL_OFFSET: u32 = 5;
            self.is_rotating = false;
            let mut dx = self.current_pos.x.wrapping_sub(self.original_anchor.x);
            if dx < 0 {
                dx = dx.wrapping_neg();
            }
            let mut dy = self.current_pos.y.wrapping_sub(self.original_anchor.y);
            if plant(9048) && dy < 0 {
                dy = dy.wrapping_neg();
            }
            // (`Int > const UnsignedInt`: both compared unsigned)
            let did_move = dx as u32 > PIXEL_OFFSET || dy as u32 > PIXEL_OFFSET;
            if !did_move && h.client_frame().wrapping_sub(self.timestamp) < CLICK_DURATION {
                h.view_set_angle_and_pitch_to_default();
                h.view_set_zoom_to_default();
            }
        } else if t == MSG_RAW_MOUSE_POSITION {
            if self.current_pos.x != msg.pos.x || self.current_pos.y != msg.pos.y {
                self.last_mouse_move_frame = h.logic_frame();
            }
            self.current_pos = msg.pos;
            let height = h.display_height();
            let width = h.display_width();
            if !h.ui_input_enabled() {
                if self.is_scrolling {
                    self.stop_scrolling(h);
                }
                return disp;
            }
            if !h.globals().windowed {
                let p = self.current_pos;
                let (hm, wm) = (height.wrapping_sub(EDGE_SCROLL_SIZE as u32), width.wrapping_sub(EDGE_SCROLL_SIZE as u32));
                if self.is_scrolling {
                    if self.scroll_type == SCROLL_SCREENEDGE && (p.x >= EDGE_SCROLL_SIZE && p.y >= EDGE_SCROLL_SIZE && lt_unsigned(p.y, hm) && lt_unsigned(p.x, wm)) {
                        self.stop_scrolling(h);
                    }
                } else if p.x < EDGE_SCROLL_SIZE || p.y < EDGE_SCROLL_SIZE || !lt_unsigned(p.y, hm) || (!lt_unsigned(p.x, wm) && !plant(9043)) {
                    self.set_scrolling(SCROLL_SCREENEDGE, h);
                }
            }
            // rotate the view
            if self.is_rotating {
                const FACTOR: f32 = 0.01;
                let angle = FACTOR * self.current_pos.x.wrapping_sub(self.anchor.x) as f32;
                let a = h.view_angle() + angle;
                h.view_set_angle(a);
                self.anchor = msg.pos;
            }
            // rotate the view up/down
            if self.is_pitching {
                const FACTOR: f32 = 0.01;
                let angle = FACTOR * self.current_pos.y.wrapping_sub(self.anchor.y) as f32;
                let a = h.view_pitch() + angle;
                h.view_set_pitch(a);
                self.anchor = msg.pos;
            }
        } else if t == MSG_RAW_MOUSE_WHEEL || t == MSG_META_OPTIONS {
            if t == MSG_RAW_MOUSE_WHEEL {
                self.last_mouse_move_frame = h.logic_frame();
                let mut spin = msg.arg1;
                if spin > 0 {
                    while spin > 0 {
                        h.view_zoom_in();
                        spin -= 1;
                    }
                } else {
                    while spin < 0 {
                        h.view_zoom_out();
                        spin += 1;
                    }
                }
                // (no break: falls into MSG_META_OPTIONS)
                if plant(9044) {
                    return disp;
                }
            }
            self.stop_scrolling(h);
        } else if t == MSG_FRAME_TICK {
            self.frame_tick(h);
        } else if (MSG_META_SAVE_VIEW1.0..=MSG_META_SAVE_VIEW8.0).contains(&t.0) {
            let slot = t.0 - MSG_META_SAVE_VIEW1.0 + 1;
            if slot > 0 && slot <= MAX_VIEW_LOCS as i32 {
                self.view_location[(slot - 1) as usize] = h.view_get_location();
                h.ui_message_bookmark_set(slot);
            }
            disp = Disposition::Destroy;
        } else if (MSG_META_VIEW_VIEW1.0..=MSG_META_VIEW_VIEW8.0).contains(&t.0) {
            let slot = t.0 - MSG_META_VIEW_VIEW1.0 + 1;
            if slot > 0 && slot <= MAX_VIEW_LOCS as i32 {
                let l = self.view_location[(slot - 1) as usize];
                h.view_set_location(&l);
            }
            disp = Disposition::Destroy;
        }
        disp
    }

    /// `translateGameMessage`'s `MSG_FRAME_TICK`: the scroll of the frame, and the replay camera.
    fn frame_tick(&mut self, h: &mut impl LookAtHost) {
        let mut offset = Coord2D { x: 0.0, y: 0.0 };
        let g = h.globals();
        if self.is_scrolling && !h.ui_is_scrolling() {
            // forced to stop scrolling (a script)
            h.ui_set_scroll_amount(offset);
            self.stop_scrolling(h);
        } else if self.is_scrolling {
            let (hf, vf, kf) = (g.horizontal_scroll_speed_factor, g.vertical_scroll_speed_factor, g.keyboard_scroll_factor);
            let amt = SCROLL_AMT as f32;
            match self.scroll_type {
                SCROLL_RMB => {
                    if h.ui_should_move_rmb_scroll_anchor() {
                        let max_x = (h.display_width() / 2) as i32;
                        let max_y = (h.display_height() / 2) as i32;
                        let (c, a) = (self.current_pos, &mut self.anchor);
                        if c.x.wrapping_add(max_x) < a.x {
                            a.x = c.x.wrapping_add(max_x);
                        } else if c.x.wrapping_sub(max_x) > a.x {
                            a.x = c.x.wrapping_sub(max_x);
                        }
                        if c.y.wrapping_add(max_y) < a.y {
                            a.y = c.y.wrapping_add(max_y);
                        } else if c.y.wrapping_sub(max_y) > a.y {
                            a.y = c.y.wrapping_sub(max_y);
                        }
                    }
                    offset.x = hf * self.current_pos.x.wrapping_sub(self.anchor.x) as f32;
                    offset.y = vf * self.current_pos.y.wrapping_sub(self.anchor.y) as f32;
                    let mut vec = offset;
                    vec.normalize();
                    // the window scroll amount as the minimum
                    offset.x += hf * vec.x * sqr(kf);
                    if !plant(9045) {
                        offset.y += vf * vec.y * sqr(kf);
                    }
                }
                SCROLL_KEY => {
                    if self.scroll_dir[DIR_UP] {
                        offset.y -= vf * amt * kf;
                    }
                    if self.scroll_dir[DIR_DOWN] {
                        offset.y += vf * amt * kf;
                    }
                    if self.scroll_dir[DIR_LEFT] {
                        offset.x -= hf * amt * kf;
                    }
                    if self.scroll_dir[DIR_RIGHT] && !plant(9046) {
                        offset.x += hf * amt * kf;
                    }
                }
                SCROLL_SCREENEDGE => {
                    let height = h.display_height();
                    let width = h.display_width();
                    let p = self.current_pos;
                    if p.y < EDGE_SCROLL_SIZE {
                        offset.y -= vf * amt * kf;
                    }
                    if !lt_unsigned(p.y, height.wrapping_sub(EDGE_SCROLL_SIZE as u32)) {
                        offset.y += vf * amt * kf;
                    }
                    if p.x < EDGE_SCROLL_SIZE {
                        offset.x -= hf * amt * kf;
                    }
                    if !lt_unsigned(p.x, width.wrapping_sub(EDGE_SCROLL_SIZE as u32)) {
                        offset.x += hf * amt * kf;
                    }
                }
                _ => {}
            }
            h.ui_set_scroll_amount(offset);
            h.view_scroll_by(&offset);
        } else {
            // not scrolling, so reset the amount
            h.ui_set_scroll_amount(offset);
        }
        if g.save_camera_in_replay && (h.in_single_player_game() || h.in_skirmish_game()) && !plant(9047) {
            let current_view = h.view_get_location();
            let cursor = h.mouse_cursor();
            h.append_replay_camera(&current_view, cursor, self.current_pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A recording host: the calls that act, as words.
    #[derive(Default)]
    struct Rec {
        input: bool,
        selecting: bool,
        ui_scrolling: bool,
        move_anchor: bool,
        shell: bool,
        stats: bool,
        frame: u32,
        cframe: u32,
        cursor: i32,
        g: LookAtGlobals,
        sp: bool,
        angle: f32,
        log: Vec<String>,
    }

    impl LookAtHost for Rec {
        fn ui_input_enabled(&self) -> bool {
            self.input
        }
        fn ui_is_selecting(&self) -> bool {
            self.selecting
        }
        fn ui_set_scrolling(&mut self, b: bool) {
            self.ui_scrolling = b;
        }
        fn ui_is_scrolling(&self) -> bool {
            self.ui_scrolling
        }
        fn ui_set_scroll_amount(&mut self, amt: Coord2D) {
            self.log.push(format!("amount {} {}", amt.x, amt.y));
        }
        fn ui_should_move_rmb_scroll_anchor(&self) -> bool {
            self.move_anchor
        }
        fn ui_message_bookmark_set(&mut self, slot: i32) {
            self.log.push(format!("bookmark {slot}"));
        }
        fn mouse_cursor(&self) -> i32 {
            self.cursor
        }
        fn mouse_set_cursor(&mut self, cursor: i32) {
            self.log.push(format!("cursor {cursor}"));
        }
        fn shell_active(&self) -> bool {
            self.shell
        }
        fn stats_collector(&self) -> bool {
            self.stats
        }
        fn stats_start_scroll_time(&mut self) {
            self.log.push("stats start".into());
        }
        fn stats_end_scroll_time(&mut self) {
            self.log.push("stats end".into());
        }
        fn logic_frame(&self) -> u32 {
            self.frame
        }
        fn client_frame(&self) -> u32 {
            self.cframe
        }
        fn display_width(&self) -> u32 {
            800
        }
        fn display_height(&self) -> u32 {
            600
        }
        fn globals(&self) -> LookAtGlobals {
            self.g
        }
        fn in_single_player_game(&self) -> bool {
            self.sp
        }
        fn in_skirmish_game(&self) -> bool {
            false
        }
        fn append_replay_camera(&mut self, loc: &ViewLocation, cursor: i32, pos: ICoord2D) {
            self.log.push(format!("replay {} {cursor} {} {}", loc.zoom, pos.x, pos.y));
        }
        fn view_set_mouse_lock(&mut self, b: bool) {
            self.log.push(format!("lock {b}"));
        }
        fn view_set_angle_and_pitch_to_default(&mut self) {
            self.log.push("default angle".into());
        }
        fn view_set_zoom_to_default(&mut self) {
            self.log.push("default zoom".into());
        }
        fn view_angle(&self) -> f32 {
            self.angle
        }
        fn view_set_angle(&mut self, a: f32) {
            self.angle = a;
            self.log.push(format!("angle {a}"));
        }
        fn view_pitch(&self) -> f32 {
            0.0
        }
        fn view_set_pitch(&mut self, a: f32) {
            self.log.push(format!("pitch {a}"));
        }
        fn view_zoom_in(&mut self) {
            self.log.push("in".into());
        }
        fn view_zoom_out(&mut self) {
            self.log.push("out".into());
        }
        fn view_scroll_by(&mut self, o: &Coord2D) {
            self.log.push(format!("scroll {} {}", o.x, o.y));
        }
        fn view_get_location(&mut self) -> ViewLocation {
            let mut l = ViewLocation::default();
            l.init(1.0, 2.0, 0.0, 0.5, 0.0, 1.5);
            l
        }
        fn view_set_location(&mut self, loc: &ViewLocation) {
            self.log.push(format!("location {} {}", loc.valid, loc.zoom));
        }
    }

    fn host() -> Rec {
        Rec { input: true, g: LookAtGlobals { windowed: true, horizontal_scroll_speed_factor: 1.0, vertical_scroll_speed_factor: 1.0, keyboard_scroll_factor: 0.5, save_camera_in_replay: false }, ..Default::default() }
    }

    fn mouse(t: crate::common::message_stream::MessageType, x: i32, y: i32) -> RawMessage {
        RawMessage::mouse(t, ICoord2D { x, y })
    }

    fn key_msg(t: crate::common::message_stream::MessageType, k: u8, up: bool) -> RawMessage {
        RawMessage { t, pos: ICoord2D::default(), delta: ICoord2D::default(), arg0: i32::from(k), arg1: if up { 1 } else { 2 } }
    }

    /// The right button scrolls by the pointer's distance from the anchor (plus the minimum) each frame, until it is
    /// released; the anchor is reported meanwhile.
    #[test]
    fn right_button_scroll() {
        let mut h = host();
        let mut x = LookAtTranslator::new();
        x.translate_game_message(&mouse(MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN, 100, 100), &mut h);
        assert!(x.is_scrolling && h.ui_scrolling);
        assert_eq!(x.get_rmb_scroll_anchor(), Some(ICoord2D { x: 100, y: 100 }));
        x.translate_game_message(&mouse(MSG_RAW_MOUSE_POSITION, 130, 100), &mut h);
        h.log.clear();
        x.translate_game_message(&RawMessage::mouse(MSG_FRAME_TICK, ICoord2D::default()), &mut h);
        assert_eq!(h.log, ["amount 30.25 0", "scroll 30.25 0"]);
        x.translate_game_message(&mouse(MSG_RAW_MOUSE_RIGHT_BUTTON_UP, 130, 100), &mut h);
        assert!(!x.is_scrolling && x.get_rmb_scroll_anchor().is_none());
        assert!(h.log.contains(&"lock false".to_string()));
    }

    /// The arrow keys scroll while held; the input off refuses the scroll; a script's stop ends it at the next tick.
    #[test]
    fn key_scroll_and_input_gate() {
        let mut h = host();
        let mut x = LookAtTranslator::new();
        x.translate_game_message(&key_msg(MSG_RAW_KEY_DOWN, key::UP, false), &mut h);
        assert_eq!(x.scroll_type, SCROLL_KEY);
        h.log.clear();
        x.translate_game_message(&RawMessage::mouse(MSG_FRAME_TICK, ICoord2D::default()), &mut h);
        assert_eq!(h.log, ["amount 0 -50", "scroll 0 -50"]);
        x.translate_game_message(&key_msg(MSG_RAW_KEY_UP, key::UP, true), &mut h);
        assert!(!x.is_scrolling);
        h.input = false;
        x.translate_game_message(&key_msg(MSG_RAW_KEY_DOWN, key::LEFT, false), &mut h);
        assert!(!x.is_scrolling, "setScrolling refuses with the input off");
        h.input = true;
        x.translate_game_message(&key_msg(MSG_RAW_KEY_DOWN, key::RIGHT, false), &mut h);
        assert!(x.is_scrolling);
        h.ui_scrolling = false;
        x.translate_game_message(&RawMessage::mouse(MSG_FRAME_TICK, ICoord2D::default()), &mut h);
        assert!(!x.is_scrolling);
        // and a mouse move with the input off stops any scroll
        x.translate_game_message(&key_msg(MSG_RAW_KEY_UP, key::LEFT, true), &mut h);
        x.translate_game_message(&key_msg(MSG_RAW_KEY_DOWN, key::DOWN, false), &mut h);
        assert!(x.is_scrolling);
        h.input = false;
        x.translate_game_message(&mouse(MSG_RAW_MOUSE_POSITION, 5, 5), &mut h);
        assert!(!x.is_scrolling);
    }

    /// Full screen: the edge starts a scroll and leaving it stops it.
    #[test]
    fn screen_edge_scroll() {
        let mut h = host();
        h.g.windowed = false;
        let mut x = LookAtTranslator::new();
        x.translate_game_message(&mouse(MSG_RAW_MOUSE_POSITION, 799, 300), &mut h);
        assert_eq!(x.scroll_type, SCROLL_SCREENEDGE);
        h.log.clear();
        x.translate_game_message(&RawMessage::mouse(MSG_FRAME_TICK, ICoord2D::default()), &mut h);
        assert_eq!(h.log, ["amount 50 0", "scroll 50 0"]);
        x.translate_game_message(&mouse(MSG_RAW_MOUSE_POSITION, 400, 300), &mut h);
        assert!(!x.is_scrolling);
    }

    /// The middle button turns the view; a click resets it; the wheel zooms and falls into the options' stop; the
    /// bookmarks save and restore; resetModes clears the modes; hasMouseMovedRecently counts a second.
    #[test]
    fn rotate_wheel_bookmarks_and_modes() {
        let mut h = host();
        let mut x = LookAtTranslator::new();
        x.translate_game_message(&mouse(MSG_RAW_MOUSE_MIDDLE_BUTTON_DOWN, 10, 10), &mut h);
        x.translate_game_message(&mouse(MSG_RAW_MOUSE_POSITION, 60, 10), &mut h);
        assert_eq!(h.angle, 0.5);
        x.translate_game_message(&mouse(MSG_RAW_MOUSE_MIDDLE_BUTTON_UP, 60, 10), &mut h);
        assert!(!h.log.contains(&"default angle".to_string()));
        x.translate_game_message(&mouse(MSG_RAW_MOUSE_MIDDLE_BUTTON_DOWN, 10, 10), &mut h);
        x.translate_game_message(&mouse(MSG_RAW_MOUSE_MIDDLE_BUTTON_UP, 10, 10), &mut h);
        assert!(h.log.contains(&"default angle".to_string()) && h.log.contains(&"default zoom".to_string()));
        h.log.clear();
        let wheel = RawMessage { t: MSG_RAW_MOUSE_WHEEL, pos: ICoord2D::default(), delta: ICoord2D::default(), arg0: 0, arg1: -2 };
        x.translate_game_message(&wheel, &mut h);
        assert_eq!(h.log, ["out", "out", "lock false", "cursor 2"]);
        h.log.clear();
        let save = RawMessage::mouse(MSG_META_SAVE_VIEW3, ICoord2D::default());
        assert_eq!(x.translate_game_message(&save, &mut h), Disposition::Destroy);
        let view = RawMessage::mouse(MSG_META_VIEW_VIEW3, ICoord2D::default());
        assert_eq!(x.translate_game_message(&view, &mut h), Disposition::Destroy);
        assert_eq!(h.log, ["bookmark 3", "location true 1.5"]);
        x.is_pitching = true;
        x.is_scrolling = true;
        x.reset_modes();
        assert!(!x.is_pitching && !x.is_scrolling && !x.is_rotating);
        x.last_mouse_move_frame = 10;
        assert!(x.has_mouse_moved_recently(40));
        assert!(!x.has_mouse_moved_recently(41));
        assert!(x.has_mouse_moved_recently(5));
        assert_eq!(x.last_mouse_move_frame, 0);
        x.set_current_pos(ICoord2D { x: 7, y: 8 });
        h.g.save_camera_in_replay = true;
        h.sp = true;
        h.log.clear();
        x.translate_game_message(&RawMessage::mouse(MSG_FRAME_TICK, ICoord2D::default()), &mut h);
        assert_eq!(h.log, ["amount 0 0", "replay 1.5 0 7 8"]);
    }
}
