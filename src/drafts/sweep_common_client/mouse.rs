//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 scripts (test: tests/differential_draft_mouse.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/Input/Mouse.cpp   (`moveMouse`, `updateMouseData`,
//!     `processMouseEvent`, `checkForDrag`; `isClick` is `crate::client::mouse::is_click`)
//!   GeneralsMD/Code/GameEngine/Include/GameClient/Mouse.h          (`MouseIO`, `MouseButtonState`)
//!
//! The verified `crate::client::mouse` holds the cursor, the tooltip and the INI. The raw event queue (`m_mouseEvents`,
//! `m_currMouse` / `m_prevMouse` and their combining) is here, as [`MouseInput`]. `getMouseEvent` is the device's
//! (DirectInput / Win32 messages) and is the [`MouseDevice`] trait.

use crate::client::window_manager::gwm;
use crate::logic::modules::plant;

/// `NUM_MOUSE_EVENTS`.
pub const NUM_MOUSE_EVENTS: usize = 256;
/// `MOUSE_MOVE_RELATIVE`.
pub const MOUSE_MOVE_RELATIVE: i32 = 0;
/// `MOUSE_MOVE_ABSOLUTE`.
pub const MOUSE_MOVE_ABSOLUTE: i32 = 1;
/// `MOUSE_NONE`.
pub const MOUSE_NONE: u8 = 0x00;
/// `MOUSE_LOST`.
pub const MOUSE_LOST: u8 = 0xFF;
/// `MOUSE_EVENT_NONE`.
pub const MOUSE_EVENT_NONE: u32 = 0x00;

/// `MouseButtonState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButtonState {
    /// `MBS_Up`.
    Up = 0,
    /// `MBS_Down`.
    Down,
    /// `MBS_DoubleClick`.
    DoubleClick,
}

impl Default for MouseButtonState {
    fn default() -> Self {
        MouseButtonState::Up
    }
}

/// `struct MouseIO`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MouseIO {
    /// `pos`.
    pub pos: (i32, i32),
    /// `time`.
    pub time: u32,
    /// `wheelPos`.
    pub wheel_pos: i32,
    /// `deltaPos`.
    pub delta_pos: (i32, i32),
    /// `leftState`.
    pub left_state: MouseButtonState,
    /// `leftEvent`.
    pub left_event: u32,
    /// `leftFrame`.
    pub left_frame: i32,
    /// `rightState`.
    pub right_state: MouseButtonState,
    /// `rightEvent`.
    pub right_event: u32,
    /// `rightFrame`.
    pub right_frame: i32,
    /// `middleState`.
    pub middle_state: MouseButtonState,
    /// `middleEvent`.
    pub middle_event: u32,
    /// `middleFrame`.
    pub middle_frame: i32,
}

/// `Mouse::getMouseEvent(result, flush)` is a pure virtual of the device mouse.
pub trait MouseDevice {
    /// Fill `result`; return `MOUSE_NONE` when nothing is left, `MOUSE_LOST` when the device was lost.
    fn get_mouse_event(&mut self, result: &mut MouseIO, flush: bool) -> u8;
}

/// The event-combining members of `class Mouse`.
#[derive(Clone, Debug)]
pub struct MouseInput {
    /// `m_mouseEvents[NUM_MOUSE_EVENTS]`.
    pub mouse_events: Vec<MouseIO>,
    /// `m_currMouse`.
    pub curr_mouse: MouseIO,
    /// `m_prevMouse`.
    pub prev_mouse: MouseIO,
    /// `m_eventsThisFrame`.
    pub events_this_frame: i32,
    /// `m_deadInputFrame`.
    pub dead_input_frame: u32,
    /// `m_inputFrame`.
    pub input_frame: u32,
    /// `m_inputMovesAbsolute`.
    pub input_moves_absolute: bool,
    /// `m_minX`, `m_maxX`, `m_minY`, `m_maxY`.
    pub min_x: i32,
    /// `m_maxX`.
    pub max_x: i32,
    /// `m_minY`.
    pub min_y: i32,
    /// `m_maxY`.
    pub max_y: i32,
    /// `updateMouseData`'s function-static `busy`.
    pub busy: bool,
}

impl MouseInput {
    /// A zeroed input state.
    pub fn new() -> MouseInput {
        MouseInput {
            mouse_events: vec![MouseIO::default(); NUM_MOUSE_EVENTS],
            curr_mouse: MouseIO::default(),
            prev_mouse: MouseIO::default(),
            events_this_frame: 0,
            dead_input_frame: 0,
            input_frame: 0,
            input_moves_absolute: false,
            min_x: 0,
            max_x: 0,
            min_y: 0,
            max_y: 0,
            busy: false,
        }
    }

    /// `Mouse::moveMouse(x, y, relOrAbs)` (Mouse.cpp:120): move the current position and clamp it to the limits.
    pub fn move_mouse(&mut self, x: i32, y: i32, rel_or_abs: i32) {
        if rel_or_abs == MOUSE_MOVE_RELATIVE {
            self.curr_mouse.pos.0 += x;
            self.curr_mouse.pos.1 += y;
        } else {
            self.curr_mouse.pos.0 = x;
            self.curr_mouse.pos.1 = y;
        }
        if plant(12301) {
            if self.curr_mouse.pos.0 < self.min_x {
                self.curr_mouse.pos.0 = self.min_x;
            } else if self.curr_mouse.pos.0 > self.max_x {
                self.curr_mouse.pos.0 = self.max_x;
            }
        } else if self.curr_mouse.pos.0 > self.max_x {
            self.curr_mouse.pos.0 = self.max_x;
        } else if self.curr_mouse.pos.0 < self.min_x {
            self.curr_mouse.pos.0 = self.min_x;
        }
        if self.curr_mouse.pos.1 > self.max_y {
            self.curr_mouse.pos.1 = self.max_y;
        } else if self.curr_mouse.pos.1 < self.min_y {
            self.curr_mouse.pos.1 = self.min_y;
        }
    }

    /// `Mouse::updateMouseData()` (Mouse.cpp:149): read the device's events into `m_mouseEvents` until it says none (a
    /// lost device is read again) or the array is full.
    pub fn update_mouse_data<D: MouseDevice + ?Sized>(&mut self, dev: &mut D) {
        let mut index: usize = 0;
        // prevent reentrancy in the event we make this mouse multi-threaded
        if !self.busy {
            self.busy = true;
            loop {
                let mut result;
                loop {
                    result = dev.get_mouse_event(&mut self.mouse_events[index], true);
                    if result != MOUSE_LOST || plant(12306) {
                        break;
                    }
                }
                index += 1;
                // `while( (result != MOUSE_NONE) && (index < sizeof( m_mouseEvents ) / sizeof( MouseIO )) )`
                if !(result != MOUSE_NONE && index < if plant(12307) { NUM_MOUSE_EVENTS - 1 } else { NUM_MOUSE_EVENTS }) {
                    break;
                }
            }
            self.busy = false;
        }
        if index > 0 {
            self.events_this_frame = if plant(12304) { index as i32 } else { index as i32 - 1 };
        } else {
            self.events_this_frame = 0;
        }
        if if plant(12305) { index > 1 } else { index != 0 } {
            self.dead_input_frame = self.input_frame;
        }
    }

    /// `Mouse::checkForDrag()` (Mouse.cpp:366): a held button whose previous event was a down or a drag is dragging.
    pub fn check_for_drag(&mut self) {
        if (if plant(12308) { self.curr_mouse.left_state == MouseButtonState::Down } else { self.curr_mouse.left_state != MouseButtonState::Up })
            && (self.prev_mouse.left_event == gwm::LEFT_DOWN || self.prev_mouse.left_event == gwm::LEFT_DRAG)
        {
            self.curr_mouse.left_event = gwm::LEFT_DRAG;
        }
        if self.curr_mouse.right_state != MouseButtonState::Up
            && (self.prev_mouse.right_event == gwm::RIGHT_DOWN || self.prev_mouse.right_event == gwm::RIGHT_DRAG)
        {
            self.curr_mouse.right_event = gwm::RIGHT_DRAG;
        }
        if self.curr_mouse.middle_state != MouseButtonState::Up
            && (self.prev_mouse.middle_event == gwm::MIDDLE_DOWN || self.prev_mouse.middle_event == gwm::MIDDLE_DRAG)
        {
            self.curr_mouse.middle_event = gwm::MIDDLE_DRAG;
        }
    }

    /// One button's block of `processMouseEvent`, the same shape for the three buttons.
    fn process_button(
        state: &mut MouseButtonState,
        event: &mut u32,
        frame: &mut i32,
        input: &MouseIO,
        in_state: MouseButtonState,
        in_frame: i32,
        prev_event: u32,
        input_frame: u32,
        down: u32,
        double: u32,
        up: u32,
        drag: u32,
    ) {
        if in_frame != 0 {
            if *state != in_state {
                if in_state == MouseButtonState::Down {
                    *event = down;
                    *state = MouseButtonState::Down;
                    *frame = if plant(12312) { in_frame } else { input_frame as i32 };
                } else if in_state == MouseButtonState::DoubleClick {
                    *event = double;
                    *state = if plant(12313) { MouseButtonState::Down } else { MouseButtonState::DoubleClick };
                    *frame = input_frame as i32;
                } else {
                    *event = up;
                    *state = MouseButtonState::Up;
                    *frame = input_frame as i32;
                }
            }
        } else if *state != MouseButtonState::Up && (prev_event == down || (prev_event == drag && !plant(12314))) {
            *event = drag;
        }
        let _ = input;
    }

    /// `Mouse::processMouseEvent(index)` (Mouse.cpp:191): combine event `index` into `m_currMouse`.
    pub fn process_mouse_event(&mut self, index: usize) {
        self.curr_mouse.left_event = MOUSE_EVENT_NONE;
        self.curr_mouse.right_event = MOUSE_EVENT_NONE;
        self.curr_mouse.middle_event = MOUSE_EVENT_NONE;
        if !plant(12310) {
            self.curr_mouse.wheel_pos = 0;
        }

        // what type of movement commands are we setup for
        let movement_type = if self.input_moves_absolute { MOUSE_MOVE_ABSOLUTE } else { MOUSE_MOVE_RELATIVE };

        self.curr_mouse.time = self.mouse_events[index].time;

        // check for drag on the first event only
        if (index == 0) != plant(12309) {
            self.check_for_drag();
        }

        let ev = self.mouse_events[index];
        self.move_mouse(ev.pos.0, ev.pos.1, movement_type);
        self.curr_mouse.wheel_pos += ev.wheel_pos;

        let input_frame = self.input_frame;
        let prev = self.prev_mouse;
        Self::process_button(
            &mut self.curr_mouse.left_state, &mut self.curr_mouse.left_event, &mut self.curr_mouse.left_frame,
            &ev, ev.left_state, ev.left_frame, prev.left_event, input_frame,
            gwm::LEFT_DOWN, gwm::LEFT_DOUBLE_CLICK, gwm::LEFT_UP, gwm::LEFT_DRAG,
        );
        Self::process_button(
            &mut self.curr_mouse.right_state, &mut self.curr_mouse.right_event, &mut self.curr_mouse.right_frame,
            &ev, ev.right_state, ev.right_frame, prev.right_event, input_frame,
            gwm::RIGHT_DOWN, gwm::RIGHT_DOUBLE_CLICK, gwm::RIGHT_UP, gwm::RIGHT_DRAG,
        );
        Self::process_button(
            &mut self.curr_mouse.middle_state, &mut self.curr_mouse.middle_event, &mut self.curr_mouse.middle_frame,
            &ev, ev.middle_state, ev.middle_frame, prev.middle_event, input_frame,
            gwm::MIDDLE_DOWN, gwm::MIDDLE_DOUBLE_CLICK, gwm::MIDDLE_UP, gwm::MIDDLE_DRAG,
        );

        self.curr_mouse.delta_pos.0 = self.curr_mouse.pos.0 - self.prev_mouse.pos.0;
        self.curr_mouse.delta_pos.1 = self.curr_mouse.pos.1 - self.prev_mouse.pos.1;
        self.prev_mouse = self.curr_mouse;
    }
}
