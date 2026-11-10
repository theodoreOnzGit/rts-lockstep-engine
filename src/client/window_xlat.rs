// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/WindowXlat.cpp (rawMouseToWindowMessage,
//     WindowTranslator::translateGameMessage)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `WindowTranslator` (GitHub #116), the message stream's first translator (priority 10): the raw mouse and key messages
//! go to the window manager first, and those a window used, or that the shell or a disabled input takes, are removed
//! from the stream before the game's translators see them.
//!
//! # As upstream reads it
//!
//! While the tactical view's mouse is locked (a drag selection or a scroll), every message is kept, except a left button
//! press or release while scrolling, which goes on to the windows (so a click on the control bar is not an order). A
//! raw mouse message (position, a button, a double click, a drag, the wheel) is given to `winProcessMouseEvent` as the
//! window message `rawMouseToWindowMessage` makes of it; its answer is overruled to "used" while the shell is active and
//! while `TheInGameUI->getInputEnabled()` is false. A raw key goes to `winProcessKey`; an unused Escape release stops a
//! movie that may be left, and with the input off is used. `MSG_META_TOGGLE_ATTACKMOVE` gives the windows a `GWM_NONE`
//! at the mouse's position and is always kept. A used message is destroyed, except a left button release while a
//! building's placement is anchored (the placement must see it).
//!
//! # Upstream quirks that are kept
//!
//! - The left button release falls through into the mouse cases, so it reaches the windows like a press.
//! - A double click is a button press to the windows (`GWM_LEFT_DOWN`, ...); the wheel is up for a positive count and
//!   down otherwise, zero included.
//! - The key's and its state's integers are truncated to bytes (`UnsignedByte`).
//! - With the input off, the escape test runs only for an unused Escape release that did not stop a movie.
//!
//! # Where this differs from upstream
//!
//! - `TheWindowManager`, `TheInGameUI` and `TheTacticalView` always exist here (in the game they do); `TheShell` is
//!   asked through the host. The `_DEBUG`/`_INTERNAL` mouse position hack is not translated.
//! - The client application has no message stream: [`translate_game_message`] is asked by the client's input glue
//!   (`client/src/input.rs`'s `Controls`) for each raw pointer event before the translators run.
//!
//! # Representation
//!
//! A raw message is a [`RawMessage`] (its type and the arguments the cases read); everything the translator asks of
//! the engine is a call on [`WindowXlatHost`]. The answer is the selection translator's [`Disposition`].

use crate::client::selection_xlat::Disposition;
use crate::client::window_manager::{gwm, key, WinInputReturnCode};
use crate::common::base_type::ICoord2D;
use crate::common::message_stream::MessageType;
use crate::common::message_types::*;
use crate::logic::modules::plant;

/// A raw input message and the arguments `WindowTranslator` reads: argument 0 is the mouse position (`pixel`) of a mouse
/// message or the key (`integer`) of a key message; argument 1 is a drag's delta (`pixel`), the wheel's count or the
/// key's state (`integer`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawMessage {
    /// `getType()`.
    pub t: MessageType,
    /// `getArgument(0)->pixel`.
    pub pos: ICoord2D,
    /// `getArgument(1)->pixel` (a drag).
    pub delta: ICoord2D,
    /// `getArgument(0)->integer` (a key).
    pub arg0: i32,
    /// `getArgument(1)->integer` (the wheel, a key's state).
    pub arg1: i32,
}

impl RawMessage {
    /// A mouse message at `pos` with no other argument.
    pub fn mouse(t: MessageType, pos: ICoord2D) -> Self {
        RawMessage { t, pos, delta: ICoord2D::default(), arg0: 0, arg1: 0 }
    }
}

/// The third argument of `winProcessMouseEvent`: `NULL`, a drag's delta, or the wheel's count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseData {
    None,
    Delta(ICoord2D),
    Wheel(i32),
}

/// What `WindowTranslator::translateGameMessage` asks of the engine.
pub trait WindowXlatHost {
    /// `TheTacticalView->isMouseLocked()`.
    fn mouse_locked(&self) -> bool;
    /// `TheInGameUI->isScrolling()`.
    fn ui_is_scrolling(&self) -> bool;
    /// `TheInGameUI->getInputEnabled()`.
    fn ui_input_enabled(&self) -> bool;
    /// `TheInGameUI->isPlacementAnchored()`.
    fn ui_placement_anchored(&self) -> bool;
    /// `TheShell && TheShell->isShellActive()`.
    fn shell_active(&self) -> bool;
    /// `TheMouse->getMouseStatus()->pos`.
    fn mouse_status_pos(&self) -> ICoord2D;
    /// `TheWindowManager->winProcessMouseEvent(gwm, &pos, data)`.
    fn win_process_mouse_event(&mut self, gwm: u32, pos: ICoord2D, data: MouseData) -> WinInputReturnCode;
    /// `TheWindowManager->winProcessKey(key, state)`.
    fn win_process_key(&mut self, key: u8, state: u8) -> WinInputReturnCode;
    /// `TheDisplay->isMoviePlaying()`.
    fn movie_playing(&self) -> bool;
    /// `TheGlobalData->m_allowExitOutOfMovies`.
    fn allow_exit_out_of_movies(&self) -> bool;
    /// `TheDisplay->stopMovie()`.
    fn stop_movie(&mut self);
}

/// `rawMouseToWindowMessage(msg)`: the window message of a raw mouse message (`GWM_NONE` for any other).
pub fn raw_mouse_to_window_message(m: &RawMessage) -> u32 {
    match m.t {
        t if t == MSG_RAW_MOUSE_POSITION => gwm::MOUSE_POS,
        t if t == MSG_RAW_MOUSE_LEFT_DOUBLE_CLICK || t == MSG_RAW_MOUSE_LEFT_BUTTON_DOWN => gwm::LEFT_DOWN,
        t if t == MSG_RAW_MOUSE_LEFT_BUTTON_UP => gwm::LEFT_UP,
        t if t == MSG_RAW_MOUSE_LEFT_DRAG => gwm::LEFT_DRAG,
        t if t == MSG_RAW_MOUSE_MIDDLE_DOUBLE_CLICK || t == MSG_RAW_MOUSE_MIDDLE_BUTTON_DOWN => gwm::MIDDLE_DOWN,
        t if t == MSG_RAW_MOUSE_MIDDLE_BUTTON_UP => gwm::MIDDLE_UP,
        t if t == MSG_RAW_MOUSE_MIDDLE_DRAG => gwm::MIDDLE_DRAG,
        t if t == MSG_RAW_MOUSE_RIGHT_DOUBLE_CLICK || t == MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN => gwm::RIGHT_DOWN,
        t if t == MSG_RAW_MOUSE_RIGHT_BUTTON_UP => gwm::RIGHT_UP,
        t if t == MSG_RAW_MOUSE_RIGHT_DRAG => gwm::RIGHT_DRAG,
        t if t == MSG_RAW_MOUSE_WHEEL => {
            if m.arg1 > 0 {
                gwm::WHEEL_UP
            } else {
                gwm::WHEEL_DOWN
            }
        }
        _ => gwm::NONE,
    }
}

/// `WindowTranslator::translateGameMessage(msg)`.
pub fn translate_game_message(m: &RawMessage, h: &mut impl WindowXlatHost) -> Disposition {
    let mut disp = Disposition::Keep;
    let mut force_keep_message = false;
    let mut return_code = WinInputReturnCode::NotUsed;

    if h.mouse_locked() {
        // (a left click while scrolling goes on to the windows, so it does not order through the control bar)
        if h.ui_is_scrolling() {
            if m.t != MSG_RAW_MOUSE_LEFT_BUTTON_UP && m.t != MSG_RAW_MOUSE_LEFT_BUTTON_DOWN {
                return Disposition::Keep;
            }
        } else {
            return Disposition::Keep;
        }
    }

    let t = m.t;
    let is_click = t == MSG_RAW_MOUSE_POSITION
        || t == MSG_RAW_MOUSE_LEFT_BUTTON_DOWN
        || t == MSG_RAW_MOUSE_LEFT_DOUBLE_CLICK
        || t == MSG_RAW_MOUSE_MIDDLE_BUTTON_DOWN
        || t == MSG_RAW_MOUSE_MIDDLE_DOUBLE_CLICK
        || t == MSG_RAW_MOUSE_MIDDLE_BUTTON_UP
        || t == MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN
        || t == MSG_RAW_MOUSE_RIGHT_DOUBLE_CLICK
        || t == MSG_RAW_MOUSE_RIGHT_BUTTON_UP;
    if t == MSG_META_TOGGLE_ATTACKMOVE {
        let mouse_pos = h.mouse_status_pos();
        h.win_process_mouse_event(gwm::NONE, mouse_pos, MouseData::None);
        return Disposition::Keep;
    } else if t == MSG_RAW_MOUSE_LEFT_BUTTON_UP || is_click {
        if t == MSG_RAW_MOUSE_LEFT_BUTTON_UP && h.ui_placement_anchored() && !plant(9026) {
            // (the release outside the windows must reach the placement)
            force_keep_message = true;
        }
        // (FALL THROUGH INTENTIONALLY)
        let gwm = raw_mouse_to_window_message(m);
        return_code = h.win_process_mouse_event(gwm, m.pos, MouseData::None);
        if h.shell_active() {
            return_code = WinInputReturnCode::Used;
        }
        if !h.ui_input_enabled() {
            return_code = WinInputReturnCode::Used;
        }
    } else if t == MSG_RAW_MOUSE_LEFT_DRAG || t == MSG_RAW_MOUSE_MIDDLE_DRAG || t == MSG_RAW_MOUSE_RIGHT_DRAG {
        let gwm = raw_mouse_to_window_message(m);
        return_code = h.win_process_mouse_event(gwm, m.pos, MouseData::Delta(m.delta));
        if h.shell_active() {
            return_code = WinInputReturnCode::Used;
        }
        if !h.ui_input_enabled() && !plant(9027) {
            return_code = WinInputReturnCode::Used;
        }
    } else if t == MSG_RAW_MOUSE_WHEEL {
        let gwm = raw_mouse_to_window_message(m);
        return_code = h.win_process_mouse_event(gwm, m.pos, MouseData::Wheel(m.arg1));
        if h.shell_active() {
            return_code = WinInputReturnCode::Used;
        }
        if !h.ui_input_enabled() {
            return_code = WinInputReturnCode::Used;
        }
    } else if t == MSG_RAW_KEY_DOWN || t == MSG_RAW_KEY_UP {
        let k = m.arg0 as u8;
        let state = m.arg1 as u8;
        return_code = h.win_process_key(k, state);
        let released = u32::from(state) & key::STATE_UP != 0;
        // (in a movie, Escape may leave it)
        if return_code != WinInputReturnCode::Used && k == key::ESC && released && h.movie_playing() && h.allow_exit_out_of_movies() {
            h.stop_movie();
            return_code = WinInputReturnCode::Used;
        }
        if return_code != WinInputReturnCode::Used && k == key::ESC && released && !h.ui_input_enabled() && !plant(9028) {
            return_code = WinInputReturnCode::Used;
        }
    }

    // (removed from the stream when a window used it)
    if return_code == WinInputReturnCode::Used && !force_keep_message {
        disp = Disposition::Destroy;
    }
    disp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Host {
        locked: bool,
        scrolling: bool,
        input_off: bool,
        anchored: bool,
        shell: bool,
        window_used: bool,
        movie: bool,
        calls: Vec<(u32, MouseData)>,
        stopped: bool,
    }
    impl WindowXlatHost for Host {
        fn mouse_locked(&self) -> bool {
            self.locked
        }
        fn ui_is_scrolling(&self) -> bool {
            self.scrolling
        }
        fn ui_input_enabled(&self) -> bool {
            !self.input_off
        }
        fn ui_placement_anchored(&self) -> bool {
            self.anchored
        }
        fn shell_active(&self) -> bool {
            self.shell
        }
        fn mouse_status_pos(&self) -> ICoord2D {
            ICoord2D { x: 9, y: 9 }
        }
        fn win_process_mouse_event(&mut self, gwm: u32, _pos: ICoord2D, data: MouseData) -> WinInputReturnCode {
            self.calls.push((gwm, data));
            if self.window_used { WinInputReturnCode::Used } else { WinInputReturnCode::NotUsed }
        }
        fn win_process_key(&mut self, _key: u8, _state: u8) -> WinInputReturnCode {
            WinInputReturnCode::NotUsed
        }
        fn movie_playing(&self) -> bool {
            self.movie
        }
        fn allow_exit_out_of_movies(&self) -> bool {
            true
        }
        fn stop_movie(&mut self) {
            self.stopped = true;
        }
    }

    /// #116: the window translator's input gate, case by case (the differential `tests/differential_input_gate.rs`
    /// compares it with WindowXlat.cpp over random cases).
    ///
    /// *Predictions, written before the first run:* with the input on and no window under the pointer a press, a drag
    /// and the wheel are kept, the press reaching the windows as `GWM_LEFT_DOWN`, the drag with its delta, the wheel
    /// down for a count of 0; with the input off each is destroyed, except a release while a placement is anchored; an
    /// Escape release with the input off is destroyed, a press of it kept; while a movie plays the Escape release stops
    /// it; with the mouse locked and no scroll nothing reaches the windows and all is kept, even with the input off.
    #[test]
    fn the_window_translator_takes_the_input_while_it_is_off() {
        let p = ICoord2D { x: 3, y: 4 };
        let down = RawMessage::mouse(MSG_RAW_MOUSE_LEFT_BUTTON_DOWN, p);
        let up = RawMessage::mouse(MSG_RAW_MOUSE_LEFT_BUTTON_UP, p);
        let drag = RawMessage { delta: ICoord2D { x: 2, y: -1 }, ..RawMessage::mouse(MSG_RAW_MOUSE_LEFT_DRAG, p) };
        let wheel = RawMessage::mouse(MSG_RAW_MOUSE_WHEEL, p);
        let esc = |t, state: u32| RawMessage { arg0: i32::from(key::ESC), arg1: state as i32, ..RawMessage::mouse(t, p) };
        let mut h = Host::default();
        for m in [down, up, drag, wheel] {
            assert_eq!(translate_game_message(&m, &mut h), Disposition::Keep, "{m:?}");
        }
        assert_eq!(h.calls, [(gwm::LEFT_DOWN, MouseData::None), (gwm::LEFT_UP, MouseData::None), (gwm::LEFT_DRAG, MouseData::Delta(drag.delta)), (gwm::WHEEL_DOWN, MouseData::Wheel(0))]);
        h.input_off = true;
        for m in [down, up, drag, wheel] {
            assert_eq!(translate_game_message(&m, &mut h), Disposition::Destroy, "{m:?}");
        }
        h.anchored = true;
        assert_eq!(translate_game_message(&up, &mut h), Disposition::Keep);
        assert_eq!(translate_game_message(&down, &mut h), Disposition::Destroy);
        assert_eq!(translate_game_message(&esc(MSG_RAW_KEY_UP, key::STATE_UP), &mut h), Disposition::Destroy);
        assert_eq!(translate_game_message(&esc(MSG_RAW_KEY_DOWN, key::STATE_DOWN), &mut h), Disposition::Keep);
        h.movie = true;
        assert_eq!(translate_game_message(&esc(MSG_RAW_KEY_UP, key::STATE_UP), &mut h), Disposition::Destroy);
        assert!(h.stopped);
        h.locked = true;
        let before = h.calls.len();
        assert_eq!(translate_game_message(&down, &mut h), Disposition::Keep);
        assert_eq!(h.calls.len(), before);
        h.scrolling = true;
        assert_eq!(translate_game_message(&down, &mut h), Disposition::Destroy, "a press while scrolling goes on to the windows");
        assert_eq!(translate_game_message(&RawMessage::mouse(MSG_RAW_MOUSE_POSITION, p), &mut h), Disposition::Keep);
        assert_eq!(translate_game_message(&RawMessage::mouse(MSG_META_TOGGLE_ATTACKMOVE, p), &mut h), Disposition::Keep);
    }
}
