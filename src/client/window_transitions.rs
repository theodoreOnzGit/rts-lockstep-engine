// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowTransitions.cpp (INI::parseWindowTransitions,
//     getTransitionForStyle, TransitionWindow, TransitionGroup, GameWindowTransitionsHandler: init, load, reset,
//     update, draw, setGroup, reverse, remove, getNewGroup, isFinished, findGroup, parseWindow, the field parse
//     table)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowTransitionsStyles.cpp (every style: Flash,
//     ButtonFlash, Fade, ScaleUp, ScoreScaleUp, MainMenuScaleUp, MainMenuMediumScaleUp, MainMenuSmallScaleDown,
//     TextType, CountUp, ScreenFade, ControlBarArrow, FullFade, TextOnFrame, ReverseSound; PushButtonImageDrawThree,
//     drawTypeText)
//   GeneralsMD/Code/GameEngine/Include/GameClient/GameWindowTransitions.h (the style enum and TransitionStyleNames,
//     each style's frame constants)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The window transitions (GitHub #77): `TheTransitionHandler`, which fades, flashes, scales and types the shell's
//! windows in and out in named **groups** read from `Data\INI\WindowTransitions.ini`.
//!
//! # As upstream reads it
//!
//! The INI's `WindowTransition <name>` blocks make the groups (`getNewGroup`: a name already there, without case, or
//! an empty one, gives none); a group's fields are `FireOnce` and any number of `Window` blocks (`WinName`, `Style`
//! from `TransitionStyleNames`, `FrameDelay`). The window manager owns the handler: `GameWindowManager::update` calls
//! its `update` after `processDestroyList`, `winRepaint` its `draw` after every window, `reset` its `reset`.
//!
//! - **A group's `init`** sets its frame to 0, its direction forward, and inits each window: the window is looked up
//!   by its name's key among every window (`winGetWindowFromId(NULL, ...)`), its old transition deleted and a new one
//!   made for its style, which records the window's place and runs its own frame 0 **backwards** (most styles hide
//!   the window there), then turns forward.
//! - **`update`** moves the group's frame by one in its direction and gives each window that frame less its delay,
//!   if that falls in `0 ..= frameLength`. Each style's frames hide or show its window, pick what it draws, play its
//!   sound; its last frame (forward) or frame 0 (backward) finishes it. **`reverse`** turns the group backward from its
//!   longest window's `delay + frameLength`; **`skip`** runs each window's last frame at once.
//! - **The handler** keeps the current group, a pending one and the two it draws (this frame's and, once, the last
//!   one's). `update`: the current group updates until finished; a finished fire-once group is dropped; a finished
//!   group gives way to the pending one; a finished reversed group is dropped. `setGroup(name, immediate)`: immediate,
//!   the current group is skipped and replaced; otherwise a current group is reversed (unless fire-once or already
//!   reversed) and the new one waits as pending; with none current it starts at once. `reverse(name)`: the current
//!   group reverses; the pending one is dropped; any other is inited, skipped and reversed as the current group.
//!   `remove(name, skipPending)` drops it from pending (skipped if asked) and from current (skipped; the pending one,
//!   if any, becomes current). `isFinished`: the current group's, or true with none.
//!
//! # Upstream quirks that are kept
//!
//! - A window the INI names but the screen lacks keeps a `NULL` window: the styles that finish only with a window
//!   (`FLASH`, `WINFADE`, ...) never finish, so the group never does, and the menu's `dontAllowTransitions` stays set.
//! - `TYPETEXT`'s frame length is its text's length (at most 30), but only frame 30 finishes it: a shorter text is
//!   finished only by a skip. `COUNTUP`'s frame length is the number's (`atoi`, by ones, hundreds or thousands); a
//!   negative number gives a negative length, and the window is never updated.
//! - `BUTTONFLASH` sets its draw state to *show background* on frames 8 to 10 without a window; `REVERSESOUND` is
//!   finished from its init, so its sound plays only when it is reversed into frame 1.
//! - `MAINMENUMEDIUMSCALEUP` and `MAINMENUSMALLSCALEDOWN` find their grow window by the window's name plus `Medium` /
//!   `Small`; `MAINMENUSCALEUP` always by `MainMenu.wnd:WinGrowMarker`, and it gives the grow window the **disabled**
//!   image of its window as its enabled image.
//! - `TransitionWindow::reverse` ignores the total it is given (its use is commented out); `getNewGroup` of a name
//!   already there gives `NULL`, into which the INI's fields are then parsed.
//! - The fields of a style left unset when it has no window are the allocator's zeros (`NEW` zero-fills).
//!
//! # Where this differs from upstream
//!
//! - Upstream's undefined behaviour is recorded in [`GameWindowManager::undefined`] (the operation stops there): a
//!   `NULL` window used (`reverse` of an unknown group with none current, `MAINMENUMEDIUMSCALEUP` without a window, a
//!   `WINFADE` frame without one, `MAINMENUSCALEUP` drawn without its grow window, `drawTypeText` on a window that is
//!   not a static text, ...), a window freed by `processDestroyList` that a transition still holds, a field parsed
//!   into the `NULL` group of a duplicate name, an `atoi` past `Int`, a centre image of width 0.
//! - Windows are [`WinId`]s; a name is looked up through the window the reader gave that name (its key), so a name
//!   no window has finds none, as `nameToKey` of a new name does. `TheControlBar->getArrowImage()`,
//!   `TheMappedImageCollection->findImageByName("Gradient")`, the images' sizes, text measuring and the sounds are the
//!   [`GuiHost`]'s.
//! - The draws are [`DrawItem`]s appended after the windows' (`winRepaint`'s order); `TheDisplay`'s clip region is
//!   [`GameWindowManager`]'s, as the W3D gadget draws use it.
//!
//! # Representation
//!
//! [`GameWindowTransitionsHandler`] is [`GameWindowManager::transitions`]: its groups (in the INI's order) and four
//! indices into them. Each [`TransitionWindow`] owns its [`Transition`], whose [`Style`] carries the style's own
//! fields. The operations are free functions over the manager and a [`GuiHost`] (the handler is taken out of the
//! manager while it runs, so that the styles can hide windows).

use crate::client::gadget::{static_text_get_text, static_text_set_text};
use crate::client::window_manager::{DrawItem, GameWindowManager, GuiHost, UserData};
use crate::client::window_script::{win_status, WinId};
use crate::common::ini::{BlockParse, FieldParse, Ini, IniLoadType, IniResult, LookupListRec, UserData as IniUserData};

/// The styles (`TRANSITION_FLASH` .. `REVERSE_SOUND_TRANSITION`).
pub mod style {
    pub const TRANSITION_FLASH: i32 = 0;
    pub const BUTTON_TRANSITION_FLASH: i32 = 1;
    pub const WIN_FADE_TRANSITION: i32 = 2;
    pub const WIN_SCALE_UP_TRANSITION: i32 = 3;
    pub const MAINMENU_SCALE_UP_TRANSITION: i32 = 4;
    pub const TEXT_TYPE_TRANSITION: i32 = 5;
    pub const SCREEN_FADE_TRANSITION: i32 = 6;
    pub const COUNT_UP_TRANSITION: i32 = 7;
    pub const FULL_FADE_TRANSITION: i32 = 8;
    pub const TEXT_ON_FRAME_TRANSITION: i32 = 9;
    pub const MAINMENU_MEDIUM_SCALE_UP_TRANSITION: i32 = 10;
    pub const MAINMENU_SMALL_SCALE_DOWN_TRANSITION: i32 = 11;
    pub const CONTROL_BAR_ARROW_TRANSITION: i32 = 12;
    pub const SCORE_SCALE_UP_TRANSITION: i32 = 13;
    pub const REVERSE_SOUND_TRANSITION: i32 = 14;
}

/// `TransitionStyleNames`.
pub const TRANSITION_STYLE_NAMES: &[LookupListRec] = &[
    LookupListRec { name: "FLASH", value: 0 },
    LookupListRec { name: "BUTTONFLASH", value: 1 },
    LookupListRec { name: "WINFADE", value: 2 },
    LookupListRec { name: "WINSCALEUP", value: 3 },
    LookupListRec { name: "MAINMENUSCALEUP", value: 4 },
    LookupListRec { name: "TYPETEXT", value: 5 },
    LookupListRec { name: "SCREENFADE", value: 6 },
    LookupListRec { name: "COUNTUP", value: 7 },
    LookupListRec { name: "FULLFADE", value: 8 },
    LookupListRec { name: "TEXTONFRAME", value: 9 },
    LookupListRec { name: "MAINMENUMEDIUMSCALEUP", value: 10 },
    LookupListRec { name: "MAINMENUSMALLSCALEDOWN", value: 11 },
    LookupListRec { name: "CONTROLBARARROW", value: 12 },
    LookupListRec { name: "SCORESCALEUP", value: 13 },
    LookupListRec { name: "REVERSESOUND", value: 14 },
];

/// The file `GameWindowTransitionsHandler::load` reads.
pub const WINDOW_TRANSITIONS_INI: &[u8] = b"Data\\INI\\WindowTransitions.ini";

// the styles' frame constants (GameWindowTransitions.h)
const FLASH_END: i32 = 8;
const BUTTONFLASH_FADE_TO_BACKGROUND_4: i32 = 7;
const BUTTONFLASH_GRADE_IN_1: i32 = 11;
const BUTTONFLASH_END: i32 = 17;
const BUTTONFLASH_SHOW_BACKGROUND: i32 = 18;
const FADE_END: i32 = 10;
const SCALEUP_END: i32 = 6;
const MAINMENU_SCALEUP_END: i32 = 5;
const MAINMENU_MEDIUM_END: i32 = 3;
const MAINMENU_SMALL_END: i32 = 6;
const TEXTTYPE_END: i32 = 30;
const COUNTUP_END: i32 = 30;
const SCREENFADE_END: i32 = 30;
const CONTROLBAR_BEGIN_FADE: i32 = 16;
const CONTROLBAR_END: i32 = 22;
const FULLFADE_END: i32 = 10;
const TEXTONFRAME_END: i32 = 1;
const REVERSESOUND_END: i32 = 2;
const COUNT_ONES: i32 = 1;
const COUNT_100S: i32 = 100;
const COUNT_1000S: i32 = 1000;

/// `ICoord2D`.
pub type Coord = (i32, i32);

/// Each style's own fields (the `Transition` subclasses').
#[derive(Clone, Debug, PartialEq)]
pub enum Style {
    Flash { pos: Coord, size: Coord, draw_state: i32 },
    /// `m_gradient`: the `Gradient` image, if the collection has it.
    ButtonFlash { pos: Coord, size: Coord, draw_state: i32, gradient: Option<Vec<u8>> },
    Fade { pos: Coord, size: Coord, draw_state: i32 },
    /// `ScaleUpTransition` and (`score`) `ScoreScaleUpTransition`, which differ only in their sound.
    ScaleUp { score: bool, pos: Coord, size: Coord, draw_state: i32, center: Coord, increment_size: Coord },
    MainMenuScaleUp { pos: Coord, size: Coord, draw_state: i32, grow_pos: Coord, grow_size: Coord, increment_pos: Coord, increment_size: Coord, grow_win: Option<WinId> },
    /// `MainMenuMediumScaleUpTransition` and (`small`) `MainMenuSmallScaleDownTransition`.
    MainMenuGrow { small: bool, pos: Coord, size: Coord, draw_state: i32, grow_pos: Coord, grow_size: Coord, increment_size: Coord, grow_win: Option<WinId> },
    TextType { pos: Coord, size: Coord, draw_state: i32, full_text: Vec<u16>, partial_text: Vec<u16> },
    CountUp { pos: Coord, size: Coord, draw_state: i32, full_text: Vec<u16>, int_value: i32, current_value: i32, count_state: i32 },
    ScreenFade { pos: Coord, size: Coord, percent: f32, draw_state: i32 },
    ControlBarArrow { pos: Coord, increment_pos: Coord, size: Coord, percent: f32, fade_percent: f32, draw_state: i32, arrow: Option<Vec<u8>> },
    FullFade { pos: Coord, size: Coord, percent: f32, draw_state: i32 },
    TextOnFrame,
    ReverseSound,
}

/// `Transition`: the fields every style has, and its own.
#[derive(Clone, Debug, PartialEq)]
pub struct Transition {
    /// `m_frameLength`.
    pub frame_length: i32,
    /// `m_isFinished` (zero until a style sets it).
    pub is_finished: bool,
    pub is_forward: bool,
    pub win: Option<WinId>,
    pub style: Style,
}

/// `getTransitionForStyle(style)`: the style's constructor (`None` for an unknown style).
pub fn get_transition_for_style(st: i32) -> Option<Transition> {
    let z = (0, 0);
    let (frame_length, s) = match st {
        style::TRANSITION_FLASH => (FLASH_END, Style::Flash { pos: z, size: z, draw_state: -1 }),
        style::BUTTON_TRANSITION_FLASH => (BUTTONFLASH_END, Style::ButtonFlash { pos: z, size: z, draw_state: -1, gradient: None }),
        style::WIN_FADE_TRANSITION => (FADE_END, Style::Fade { pos: z, size: z, draw_state: -1 }),
        style::WIN_SCALE_UP_TRANSITION | style::SCORE_SCALE_UP_TRANSITION => {
            (SCALEUP_END, Style::ScaleUp { score: st == style::SCORE_SCALE_UP_TRANSITION, pos: z, size: z, draw_state: -1, center: z, increment_size: z })
        }
        style::MAINMENU_SCALE_UP_TRANSITION => (
            MAINMENU_SCALEUP_END,
            Style::MainMenuScaleUp { pos: z, size: z, draw_state: -1, grow_pos: z, grow_size: z, increment_pos: z, increment_size: z, grow_win: None },
        ),
        style::MAINMENU_MEDIUM_SCALE_UP_TRANSITION => {
            (MAINMENU_MEDIUM_END, Style::MainMenuGrow { small: false, pos: z, size: z, draw_state: -1, grow_pos: z, grow_size: z, increment_size: z, grow_win: None })
        }
        style::MAINMENU_SMALL_SCALE_DOWN_TRANSITION => {
            (MAINMENU_SMALL_END, Style::MainMenuGrow { small: true, pos: z, size: z, draw_state: -1, grow_pos: z, grow_size: z, increment_size: z, grow_win: None })
        }
        style::TEXT_TYPE_TRANSITION => (TEXTTYPE_END, Style::TextType { pos: z, size: z, draw_state: -1, full_text: Vec::new(), partial_text: Vec::new() }),
        style::SCREEN_FADE_TRANSITION => (SCREENFADE_END, Style::ScreenFade { pos: z, size: z, percent: 0.0, draw_state: -1 }),
        style::COUNT_UP_TRANSITION => {
            (COUNTUP_END, Style::CountUp { pos: z, size: z, draw_state: -1, full_text: Vec::new(), int_value: 0, current_value: 0, count_state: 0 })
        }
        style::FULL_FADE_TRANSITION => (FULLFADE_END, Style::FullFade { pos: z, size: z, percent: 0.0, draw_state: -1 }),
        style::TEXT_ON_FRAME_TRANSITION => (TEXTONFRAME_END, Style::TextOnFrame),
        style::REVERSE_SOUND_TRANSITION => (REVERSESOUND_END, Style::ReverseSound),
        style::CONTROL_BAR_ARROW_TRANSITION => {
            (CONTROLBAR_END, Style::ControlBarArrow { pos: z, increment_pos: z, size: z, percent: 0.0, fade_percent: 0.0, draw_state: -1, arrow: None })
        }
        _ => return None,
    };
    Some(Transition { frame_length, is_finished: false, is_forward: true, win: None, style: s })
}

/// `TransitionWindow`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransitionWindow {
    /// The INI's `WinName`, `FrameDelay`, `Style`.
    pub win_name: Vec<u8>,
    pub frame_delay: i32,
    pub style: i32,
    pub win: Option<WinId>,
    /// `m_transition` (`None` until the group's first init).
    pub transition: Option<Transition>,
    pub current_frame_delay: i32,
}

/// `TransitionGroup`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransitionGroup {
    pub name: Vec<u8>,
    pub fire_once: bool,
    pub windows: Vec<TransitionWindow>,
    /// `m_directionMultiplier` (zero until the first `init`).
    pub direction_multiplier: i32,
    pub current_frame: i32,
}

/// `GameWindowTransitionsHandler`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GameWindowTransitionsHandler {
    /// `m_transitionGroupList`, in the INI's order.
    pub groups: Vec<TransitionGroup>,
    pub current: Option<usize>,
    pub pending: Option<usize>,
    pub draw_group: Option<usize>,
    pub secondary_draw_group: Option<usize>,
    /// Upstream's undefined behaviour met while parsing (a field into the `NULL` group of a duplicate name).
    pub undefined: Option<&'static str>,
}

// ---------------------------------------------------------------------------
// the window and the host
// ---------------------------------------------------------------------------

/// A window a transition holds, or `None` (recorded) if `processDestroyList` has freed it: upstream's pointer dangles.
fn live(wm: &mut GameWindowManager, w: WinId) -> Option<WinId> {
    if wm.ext[w].freed {
        wm.set_undefined("window transitions: a window freed by processDestroyList is still held by a transition");
        return None;
    }
    Some(w)
}

/// `win->...` on a window upstream does not test for `NULL` (recorded when it is, or freed).
fn must(wm: &mut GameWindowManager, w: Option<WinId>, what: &'static str) -> Option<WinId> {
    match w {
        Some(w) => live(wm, w),
        None => {
            wm.set_undefined(what);
            None
        }
    }
}

fn hide(wm: &mut GameWindowManager, w: Option<WinId>, h: bool, what: &'static str) {
    if let Some(w) = must(wm, w, what) {
        wm.win_hide(w, h);
    }
}

/// `winGetSize` and `winGetScreenPosition`.
fn place(wm: &GameWindowManager, w: WinId) -> (Coord, Coord) {
    (wm.tree.screen_position(w), wm.tree.windows[w].size)
}

/// `TheWindowManager->winGetWindowFromId(NULL, TheNameKeyGenerator->nameToKey(name))`: the key is the one the reader
/// gave the windows of that name; a name no window has has a key no window has.
pub fn find_window(wm: &GameWindowManager, name: &[u8]) -> Option<WinId> {
    if name.is_empty() {
        return None;
    }
    let key = wm.tree.windows.iter().find(|w| w.inst.decorated_name_string == name)?.id;
    wm.win_get_window_from_id(None, key)
}

/// `GameMakeColor(r, g, b, a)` (each an `UnsignedByte`).
pub fn game_make_color(r: u8, g: u8, b: u8, a: u8) -> i32 {
    ((a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32) as i32
}

fn fill(out: &mut Vec<DrawItem>, x: i32, y: i32, w: i32, h: i32, color: i32) {
    out.push(DrawItem::FillRect { x, y, w, h, color });
}
fn open(out: &mut Vec<DrawItem>, x: i32, y: i32, w: i32, h: i32, color: i32) {
    out.push(DrawItem::OpenRect { x, y, w, h, color });
}
/// `TheDisplay->drawImage(image, x0, y0, x1, y1, color)` (a `NULL` image draws nothing).
#[allow(clippy::too_many_arguments)]
fn image(wm: &GameWindowManager, out: &mut Vec<DrawItem>, img: Option<&Vec<u8>>, x0: i32, y0: i32, x1: i32, y1: i32, color: i32) {
    if let Some(i) = img {
        out.push(DrawItem::Image { image: i.clone(), x0, y0, x1, y1, color: color as u32, grayscale: false, clip: wm.clip });
    }
}

fn audio<H: GuiHost>(h: &mut H, name: &[u8]) {
    h.add_audio_event(name);
}

/// `atoi` of `AsciiString::translate(text)` (each unit cast to `char`): `None` past `Int` (undefined).
fn atoi(text: &[u16]) -> Option<i32> {
    let b: Vec<u8> = text.iter().map(|&c| c as u8).collect();
    let mut i = 0;
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r') {
        i += 1;
    }
    let neg = i < b.len() && b[i] == b'-';
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    let mut v: i64 = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        v = v * 10 + i64::from(b[i] - b'0');
        if v > i64::from(i32::MAX) + 1 {
            return None;
        }
        i += 1;
    }
    let v = if neg { -v } else { v };
    i32::try_from(v).ok()
}

/// `UnicodeString::format(L"%d", v)`.
fn format_int(v: i32) -> Vec<u16> {
    v.to_string().encode_utf16().collect()
}

// ---------------------------------------------------------------------------
// the styles
// ---------------------------------------------------------------------------

impl Transition {
    fn draw_state_mut(&mut self) -> Option<&mut i32> {
        match &mut self.style {
            Style::Flash { draw_state, .. }
            | Style::ButtonFlash { draw_state, .. }
            | Style::Fade { draw_state, .. }
            | Style::ScaleUp { draw_state, .. }
            | Style::MainMenuScaleUp { draw_state, .. }
            | Style::MainMenuGrow { draw_state, .. }
            | Style::TextType { draw_state, .. }
            | Style::CountUp { draw_state, .. }
            | Style::ScreenFade { draw_state, .. }
            | Style::ControlBarArrow { draw_state, .. }
            | Style::FullFade { draw_state, .. } => Some(draw_state),
            Style::TextOnFrame | Style::ReverseSound => None,
        }
    }

    fn set_draw_state(&mut self, v: i32) {
        if let Some(d) = self.draw_state_mut() {
            *d = v;
        }
    }

    /// `if(win) { m_win = win; winGetSize(&m_size); winGetScreenPosition(&m_pos); }`.
    fn take_window(&mut self, wm: &GameWindowManager, win: Option<WinId>) {
        let Some(w) = win else { return };
        self.win = Some(w);
        let (p, s) = place(wm, w);
        match &mut self.style {
            Style::Flash { pos, size, .. }
            | Style::ButtonFlash { pos, size, .. }
            | Style::Fade { pos, size, .. }
            | Style::ScaleUp { pos, size, .. }
            | Style::MainMenuScaleUp { pos, size, .. }
            | Style::MainMenuGrow { pos, size, .. }
            | Style::TextType { pos, size, .. }
            | Style::CountUp { pos, size, .. }
            | Style::FullFade { pos, size, .. } => {
                *pos = p;
                *size = s;
            }
            _ => {}
        }
    }

    /// The common opening of most styles' `init`: the window taken, frame 0 run backwards, then forward.
    fn init_backwards<H: GuiHost>(&mut self, wm: &mut GameWindowManager, h: &mut H) {
        self.is_forward = false;
        self.update(wm, h, 0);
        self.is_finished = false;
        self.is_forward = true;
    }

    /// `init(win)`.
    pub fn init<H: GuiHost>(&mut self, wm: &mut GameWindowManager, h: &mut H, win: Option<WinId>) {
        match self.style.clone() {
            Style::Flash { .. } | Style::Fade { .. } | Style::FullFade { .. } => {
                self.take_window(wm, win);
                self.init_backwards(wm, h);
                if let Style::FullFade { percent, .. } = &mut self.style {
                    *percent = 1.0f32 / (FULLFADE_END / 2) as f32;
                }
            }
            Style::ButtonFlash { .. } => {
                self.take_window(wm, win);
                self.init_backwards(wm, h);
                let g = h.has_image(b"Gradient").then(|| b"Gradient".to_vec());
                if let Style::ButtonFlash { gradient, .. } = &mut self.style {
                    *gradient = g;
                }
            }
            Style::ScaleUp { .. } => {
                self.take_window(wm, win);
                self.init_backwards(wm, h);
                if let Style::ScaleUp { pos, size, center, increment_size, .. } = &mut self.style {
                    *center = (pos.0.wrapping_add(size.0 / 2), pos.1.wrapping_add(size.1 / 2));
                    *increment_size = (size.0 / SCALEUP_END, size.1 / SCALEUP_END);
                }
            }
            Style::MainMenuScaleUp { .. } => {
                self.take_window(wm, win);
                let gw = find_window(wm, b"MainMenu.wnd:WinGrowMarker");
                if let Style::MainMenuScaleUp { grow_win, .. } = &mut self.style {
                    *grow_win = gw;
                }
                let Some(g) = gw else { return };
                let (gp, gs) = place(wm, g);
                if let Style::MainMenuScaleUp { grow_pos, grow_size, .. } = &mut self.style {
                    *grow_pos = gp;
                    *grow_size = gs;
                }
                self.init_backwards(wm, h);
                if let Style::MainMenuScaleUp { pos, size, grow_pos, grow_size, increment_pos, increment_size, .. } = &mut self.style {
                    *increment_pos = (grow_pos.0.wrapping_sub(pos.0) / MAINMENU_SCALEUP_END, grow_pos.1.wrapping_sub(pos.1) / MAINMENU_SCALEUP_END);
                    *increment_size = (grow_size.0.wrapping_sub(size.0) / MAINMENU_SCALEUP_END, grow_size.1.wrapping_sub(size.1) / MAINMENU_SCALEUP_END);
                }
                let Some(w) = must(wm, self.win, "MainMenuScaleUpTransition::init: no window (m_win->winGetDisabledImage)") else { return };
                let img = wm.tree.windows[w].inst.disabled_draw_data[0].image.clone();
                if let Some(g) = live(wm, g) {
                    wm.tree.windows[g].inst.enabled_draw_data[0].image = img;
                }
            }
            Style::MainMenuGrow { small, .. } => {
                self.take_window(wm, win);
                let Some(w) = must(wm, self.win, "MainMenu*ScaleTransition::init: no window (m_win->winGetInstanceData)") else { return };
                let mut name = wm.tree.windows[w].inst.decorated_name_string.clone();
                name.extend_from_slice(if small { b"Small" } else { b"Medium" });
                let gw = find_window(wm, &name);
                if let Style::MainMenuGrow { grow_win, .. } = &mut self.style {
                    *grow_win = gw;
                }
                let Some(g) = gw else { return };
                let (gp, gs) = place(wm, g);
                if let Style::MainMenuGrow { grow_pos, grow_size, .. } = &mut self.style {
                    *grow_pos = gp;
                    *grow_size = gs;
                }
                self.init_backwards(wm, h);
                let end = if small { MAINMENU_SMALL_END } else { MAINMENU_MEDIUM_END };
                if let Style::MainMenuGrow { size, grow_size, increment_size, .. } = &mut self.style {
                    *increment_size = (grow_size.0.wrapping_sub(size.0) / end, grow_size.1.wrapping_sub(size.1) / end);
                }
                if small {
                    let img = wm.tree.windows[w].inst.enabled_draw_data[0].image.clone();
                    wm.tree.windows[g].inst.enabled_draw_data[0].image = img;
                }
            }
            Style::TextType { .. } => {
                self.take_window(wm, win);
                self.init_backwards(wm, h);
                // (`m_dStr = TheDisplayStringManager->newDisplayString()`: the partial text below)
                let full = static_text(wm, self.win);
                let len = full.len() as i32;
                if let Style::TextType { full_text, .. } = &mut self.style {
                    *full_text = full;
                }
                self.frame_length = len.min(TEXTTYPE_END);
            }
            Style::CountUp { .. } => {
                if let Some(w) = win {
                    self.take_window(wm, Some(w));
                    if wm.tree.windows[w].is_hidden() {
                        self.is_forward = true;
                        self.is_finished = true;
                        self.frame_length = 0;
                        return;
                    }
                }
                let full = static_text(wm, self.win);
                if let Style::CountUp { full_text, .. } = &mut self.style {
                    *full_text = full.clone();
                }
                self.init_backwards(wm, h);
                let Some(v) = atoi(&full) else {
                    wm.set_undefined("CountUpTransition::init: atoi of a number past Int");
                    return;
                };
                let (state, len) = if v < COUNTUP_END {
                    (COUNT_ONES, v.min(COUNTUP_END))
                } else if v / 100 < COUNTUP_END {
                    (COUNT_100S, (v / 100).min(COUNTUP_END))
                } else {
                    (COUNT_1000S, (v / 1000).min(COUNTUP_END))
                };
                self.frame_length = len;
                if let Style::CountUp { int_value, count_state, current_value, .. } = &mut self.style {
                    *int_value = v;
                    *count_state = state;
                    *current_value = 0;
                }
                static_text_set_text(wm, h, self.win, &format_int(0));
            }
            Style::ScreenFade { .. } => {
                self.init_backwards(wm, h);
                let d = wm.display;
                if let Style::ScreenFade { percent, pos, size, .. } = &mut self.style {
                    *percent = 1.0f32 / (SCREENFADE_END - 1 + i32::from(crate::logic::modules::plant(4714))) as f32;
                    *pos = (0, 0);
                    *size = d;
                }
            }
            Style::ControlBarArrow { .. } => {
                self.init_backwards(wm, h);
                let a = h.control_bar_arrow_image();
                let twin = find_window(wm, b"ControlBar.wnd:ButtonGeneral");
                let asize = a.as_ref().map(|i| h.image_size(i).unwrap_or((0, 0)));
                let tplace = twin.map(|t| place(wm, t));
                let Style::ControlBarArrow { pos, increment_pos, size, percent, fade_percent, arrow, .. } = &mut self.style else { return };
                *percent = 1.0f32 / CONTROLBAR_BEGIN_FADE as f32;
                *fade_percent = 1.0f32 / (CONTROLBAR_END - CONTROLBAR_BEGIN_FADE) as f32;
                *arrow = a;
                let (Some((sp, ss)), Some((aw, ah))) = (tplace, asize) else {
                    self.is_finished = true;
                    return;
                };
                *increment_pos = (0, (sp.1 as f32 * *percent) as i32);
                *pos = ((sp.0 + ss.0 / 2) - aw / 2, 0 - ah + 20);
                *size = (aw, ah);
            }
            Style::TextOnFrame => {
                if win.is_some() {
                    self.win = win;
                }
                let Some(w) = must(wm, self.win, "TextOnFrameTransition::init: no window (m_win->winIsHidden)") else { return };
                if wm.tree.windows[w].is_hidden() {
                    self.is_finished = true;
                    self.is_forward = true;
                    self.frame_length = 0;
                } else {
                    self.init_backwards(wm, h);
                }
            }
            Style::ReverseSound => {
                self.is_finished = true;
                self.is_forward = true;
            }
        }
    }

    /// `update(frame)`.
    pub fn update<H: GuiHost>(&mut self, wm: &mut GameWindowManager, h: &mut H, frame: i32) {
        let fwd = self.is_forward;
        let win = self.win;
        if !matches!(self.style, Style::TextOnFrame | Style::ReverseSound) {
            self.set_draw_state(-1);
        }
        if frame < 0 || frame > self.max_frame() {
            return;
        }
        match self.style.clone() {
            Style::Flash { .. } => match frame {
                0 => {
                    if !fwd && win.is_some() {
                        hide(wm, win, true, "");
                        self.is_finished = true;
                    }
                }
                1..=3 => {
                    if frame == 1 && fwd {
                        audio(h, b"GUIBoarderFadeIn");
                    }
                    if win.is_some() {
                        hide(wm, win, true, "");
                        self.set_draw_state(frame);
                    }
                }
                4..=7 => {
                    if win.is_some() {
                        hide(wm, win, false, "");
                        self.set_draw_state(frame);
                    }
                }
                _ => {
                    if fwd && win.is_some() {
                        hide(wm, win, false, "");
                        self.is_finished = true;
                    }
                }
            },
            Style::ButtonFlash { .. } => {
                match frame {
                    0 => {
                        if !fwd && win.is_some() {
                            hide(wm, win, true, "");
                            self.is_finished = true;
                        }
                    }
                    1..=7 => {
                        if win.is_some() {
                            hide(wm, win, true, "");
                            if frame == 1 && fwd {
                                audio(h, b"GUIButtonsFadeIn");
                            }
                            // backwards: FADE_IN_1..3 draw FADE_TO_BACKGROUND_4..2, BACKGROUND_1 itself,
                            // BACKGROUND_2..4 draw FADE_IN_3..1
                            let back = if crate::logic::modules::plant(4711) { [0, 7, 6, 5, 3, 3, 2, 1] } else { [0, 7, 6, 5, 4, 3, 2, 1] };
                            self.set_draw_state(if fwd { frame } else { back[frame as usize] });
                        }
                    }
                    11..=16 => {
                        if win.is_some() {
                            if fwd {
                                hide(wm, win, false, "");
                                self.set_draw_state(frame);
                            } else {
                                // GRADE_OUT_3 and _4 backwards show; the rest hide
                                hide(wm, win, frame < 15, "");
                                self.set_draw_state(27 - frame);
                            }
                        }
                    }
                    BUTTONFLASH_END if fwd && win.is_some() => {
                        hide(wm, win, false, "");
                        self.is_finished = true;
                    }
                    _ => {}
                }
                if frame > BUTTONFLASH_FADE_TO_BACKGROUND_4 && frame < BUTTONFLASH_GRADE_IN_1 {
                    self.set_draw_state(BUTTONFLASH_SHOW_BACKGROUND);
                }
            }
            Style::Fade { .. } => match frame {
                0 => {
                    if !fwd && win.is_some() {
                        hide(wm, win, true, "");
                        self.is_finished = true;
                    }
                }
                1..=9 => {
                    hide(wm, win, true, "FadeTransition::update: no window (m_win->winHide)");
                    self.set_draw_state(frame);
                }
                _ => {
                    if fwd && win.is_some() {
                        hide(wm, win, false, "");
                        self.is_finished = true;
                    }
                }
            },
            Style::ScaleUp { score, .. } => match frame {
                0 => {
                    if !fwd && win.is_some() {
                        hide(wm, win, true, "");
                        self.is_finished = true;
                    }
                }
                1..=5 => {
                    if frame == 1 && fwd {
                        audio(h, if score { b"GUIScoreScreenPictures".as_slice() } else { b"GUILogoMouseOver" });
                    }
                    if win.is_some() {
                        hide(wm, win, true, "");
                    }
                    self.set_draw_state(frame);
                }
                _ => {
                    if fwd && win.is_some() {
                        hide(wm, win, false, "");
                        self.is_finished = true;
                    }
                }
            },
            Style::MainMenuScaleUp { grow_win, .. } => {
                let both = win.is_some() && grow_win.is_some();
                if frame == 0 && !fwd && both {
                    hide(wm, grow_win, true, "");
                    self.is_finished = true;
                } else if frame == MAINMENU_SCALEUP_END && fwd && both {
                    hide(wm, win, true, "");
                    hide(wm, grow_win, false, "");
                    self.is_finished = true;
                }
                if frame == 1 {
                    audio(h, b"GUILogoSelect");
                }
                if frame > 0 && frame < MAINMENU_SCALEUP_END {
                    if win.is_some() {
                        hide(wm, win, true, "");
                    }
                    if grow_win.is_some() {
                        hide(wm, grow_win, true, "");
                    }
                    self.set_draw_state(frame);
                }
            }
            Style::MainMenuGrow { small, grow_win, .. } => {
                let end = if small { MAINMENU_SMALL_END } else { MAINMENU_MEDIUM_END };
                let both = win.is_some() && grow_win.is_some();
                if frame == 0 {
                    if !fwd && both {
                        hide(wm, win, false, "");
                        hide(wm, grow_win, true, "");
                        self.is_finished = true;
                    }
                } else if frame == end {
                    if fwd && both {
                        hide(wm, win, true, "");
                        hide(wm, grow_win, false, "");
                        self.is_finished = true;
                    }
                } else {
                    if !small && frame == 1 && fwd {
                        audio(h, b"GUILogoMouseOver");
                    }
                    if win.is_some() {
                        hide(wm, win, true, "");
                    }
                    if grow_win.is_some() {
                        hide(wm, grow_win, true, "");
                    }
                    self.set_draw_state(frame);
                }
            }
            Style::TextType { full_text, .. } => {
                if frame == 0 && !fwd && win.is_some() {
                    hide(wm, win, true, "");
                    self.is_finished = true;
                } else if frame == TEXTTYPE_END && fwd && win.is_some() {
                    hide(wm, win, false, "");
                    self.is_finished = true;
                }
                if frame >= self.frame_length {
                    hide(wm, win, false, "TextTypeTransition::update: no window (m_win->winHide)");
                }
                if frame > 0 && frame < self.frame_length {
                    hide(wm, win, true, "TextTypeTransition::update: no window (m_win->winHide)");
                    self.set_draw_state(frame);
                    audio(h, b"GUITypeText");
                    if let Style::TextType { partial_text, .. } = &mut self.style {
                        if fwd {
                            partial_text.push(full_text[(frame - 1) as usize]);
                        } else {
                            partial_text.pop();
                        }
                    }
                }
            }
            Style::CountUp { full_text, int_value, count_state, mut current_value, .. } => {
                if frame == 0 && !fwd && win.is_some() {
                    if let Style::CountUp { current_value, .. } = &mut self.style {
                        *current_value = 0;
                    }
                    static_text_set_text(wm, h, win, &format_int(0));
                    hide(wm, win, true, "");
                    self.is_finished = true;
                } else if frame == COUNTUP_END && fwd && win.is_some() {
                    hide(wm, win, false, "");
                    self.is_finished = true;
                }
                if frame >= self.frame_length {
                    hide(wm, win, false, "CountUpTransition::update: no window (m_win->winHide)");
                }
                if frame > 0 && frame < self.frame_length {
                    hide(wm, win, false, "CountUpTransition::update: no window (m_win->winHide)");
                    self.set_draw_state(frame);
                    audio(h, b"GUIScoreScreenTick");
                    if let Style::CountUp { current_value: cv, .. } = &self.style {
                        current_value = *cv;
                    }
                    current_value = current_value.wrapping_add(count_state);
                    if current_value > int_value {
                        current_value = int_value;
                    }
                    if let Style::CountUp { current_value: cv, .. } = &mut self.style {
                        *cv = current_value;
                    }
                    static_text_set_text(wm, h, win, &format_int(current_value));
                }
                if frame == self.frame_length {
                    static_text_set_text(wm, h, win, &full_text);
                    self.is_finished = true;
                }
            }
            Style::ScreenFade { .. } | Style::ControlBarArrow { .. } => {
                if frame == 0 || frame == self.max_frame() {
                    self.is_finished = true;
                }
                self.set_draw_state(frame);
            }
            Style::FullFade { .. } => {
                if frame == 0 && !fwd && win.is_some() {
                    hide(wm, win, true, "");
                    self.is_finished = true;
                } else if frame == FULLFADE_END && fwd && win.is_some() {
                    hide(wm, win, false, "");
                    self.is_finished = true;
                }
                if frame == FULLFADE_END / 2 {
                    hide(wm, win, !fwd, "FullFadeTransition::update: no window (m_win->winHide)");
                }
                self.set_draw_state(frame);
            }
            Style::TextOnFrame => {
                if frame == 0 && !fwd && win.is_some() {
                    hide(wm, win, true, "");
                    self.is_finished = true;
                } else if frame == TEXTONFRAME_END && fwd && win.is_some() {
                    hide(wm, win, false, "");
                    self.is_finished = true;
                }
            }
            Style::ReverseSound => {
                if frame == 0 {
                    if !fwd {
                        self.is_finished = true;
                    }
                } else {
                    if frame == 1 {
                        audio(h, b"GUITransitionFade");
                    }
                    if fwd {
                        self.is_finished = true;
                    }
                }
            }
        }
    }

    /// The last frame `update` takes (each style's `*_END`, whatever its frame length).
    fn max_frame(&self) -> i32 {
        match self.style {
            Style::Flash { .. } => FLASH_END,
            Style::ButtonFlash { .. } => BUTTONFLASH_END,
            Style::Fade { .. } => FADE_END,
            Style::ScaleUp { .. } => SCALEUP_END,
            Style::MainMenuScaleUp { .. } => MAINMENU_SCALEUP_END,
            Style::MainMenuGrow { small, .. } => {
                if small {
                    MAINMENU_SMALL_END
                } else {
                    MAINMENU_MEDIUM_END
                }
            }
            Style::TextType { .. } => TEXTTYPE_END,
            Style::CountUp { .. } => COUNTUP_END,
            Style::ScreenFade { .. } => SCREENFADE_END,
            Style::ControlBarArrow { .. } => CONTROLBAR_END,
            Style::FullFade { .. } => FULLFADE_END,
            Style::TextOnFrame => TEXTONFRAME_END,
            Style::ReverseSound => REVERSESOUND_END,
        }
    }

    /// `reverse()`.
    pub fn reverse(&mut self, wm: &mut GameWindowManager) {
        match &mut self.style {
            Style::TextType { full_text, partial_text, .. } => {
                *partial_text = full_text.clone();
            }
            Style::CountUp { .. } => {
                if let Some(w) = must(wm, self.win, "CountUpTransition::reverse: no window (m_win->winIsHidden)") {
                    if wm.tree.windows[w].is_hidden() {
                        self.is_forward = false;
                        self.is_finished = true;
                        self.frame_length = 0;
                        return;
                    }
                } else {
                    return;
                }
            }
            Style::MainMenuGrow { small: false, grow_win, .. } => {
                let g = *grow_win;
                self.is_finished = false;
                self.is_forward = false;
                hide(wm, self.win, true, "MainMenuMediumScaleUpTransition::reverse: no window (m_win->winHide)");
                hide(wm, g, true, "MainMenuMediumScaleUpTransition::reverse: no grow window (m_growWin->winHide)");
                return;
            }
            Style::TextOnFrame => {
                self.is_finished = false;
                self.is_forward = false;
                if let Some(w) = must(wm, self.win, "TextOnFrameTransition::reverse: no window (m_win->winIsHidden)") {
                    if wm.tree.windows[w].is_hidden() {
                        self.is_finished = true;
                        self.frame_length = 0;
                    }
                }
                return;
            }
            _ => {}
        }
        self.is_finished = false;
        self.is_forward = false;
    }

    /// `skip()`.
    pub fn skip<H: GuiHost>(&mut self, wm: &mut GameWindowManager, h: &mut H) {
        if matches!(self.style, Style::CountUp { .. } | Style::TextOnFrame | Style::ReverseSound) && self.is_finished {
            return;
        }
        let end = self.max_frame();
        self.update(wm, h, end);
    }

    /// `draw()`.
    pub fn draw<H: GuiHost>(&self, wm: &mut GameWindowManager, h: &mut H, out: &mut Vec<DrawItem>) {
        let fwd = self.is_forward;
        match &self.style {
            Style::Flash { pos, size, draw_state } => {
                let (open_a, fill_a) = match draw_state {
                    1 => (100, 33),
                    2 => (150, 66),
                    3 => (200, 99),
                    4 => (250, 75),
                    5 => (250, 50),
                    6 => (250, 25),
                    7 => (250, 10),
                    _ => return,
                };
                let (x, y, w, hh) = (pos.0 + 1, pos.1 + 1, size.0 - 2, size.1);
                open(out, x, y, w, hh, game_make_color(255, 255, 255, open_a));
                fill(out, x, y, w, hh, game_make_color(255, 255, 255, fill_a));
            }
            Style::ButtonFlash { pos, size, draw_state, gradient } => {
                let (x, y, w, hh) = (pos.0, pos.1, size.0, size.1);
                let rects = |out: &mut Vec<DrawItem>, oa: u8, fa: u8| {
                    open(out, x, y, w, hh, game_make_color(255, 255, 255, oa));
                    fill(out, x, y, w, hh, game_make_color(255, 255, 255, fa));
                };
                let ds = *draw_state;
                match ds {
                    1 => rects(out, 100, 75),
                    2 => rects(out, 150, 150),
                    3 => rects(out, 200, 200),
                    4..=7 => {
                        push_button_image_draw_three(wm, h, self.win, 255, out);
                        let fa = [150, 100, 50, 15][(ds - 4) as usize];
                        rects(out, 250, fa);
                    }
                    11..=16 => {
                        let three = match ds {
                            11 => fwd,
                            12 => false,
                            _ => !fwd,
                        };
                        if three {
                            push_button_image_draw_three(wm, h, self.win, 255, out);
                        }
                        let a = [100, 200, 150, 100, 50, 17][(ds - 11) as usize];
                        image(wm, out, gradient.as_ref(), x, y, x + w, y + hh, game_make_color(255, 255, 255, a));
                    }
                    BUTTONFLASH_SHOW_BACKGROUND => push_button_image_draw_three(wm, h, self.win, 255, out),
                    _ => {}
                }
            }
            Style::Fade { pos, size, draw_state } => {
                let Some(w) = self.win.and_then(|w| live(wm, w)) else { return };
                let img = wm.tree.windows[w].inst.enabled_draw_data[0].image.clone();
                if (1..=9).contains(draw_state) {
                    let a = (25 * draw_state) as u8;
                    image(wm, out, img.as_ref(), pos.0, pos.1, pos.0 + size.0, pos.1 + size.1, game_make_color(255, 255, 255, a));
                }
            }
            Style::ScaleUp { draw_state, center, increment_size, .. } => {
                let Some(w) = self.win.and_then(|w| live(wm, w)) else { return };
                let img = wm.tree.windows[w].inst.enabled_draw_data[0].image.clone();
                let ds = *draw_state;
                if ds <= 0 || ds >= SCALEUP_END {
                    return;
                }
                let x = center.0 - (increment_size.0 * ds) / 2;
                let y = center.1 - (increment_size.1 * ds) / 2;
                image(wm, out, img.as_ref(), x, y, x + increment_size.0 * ds, y + increment_size.1 * ds, -1);
            }
            Style::MainMenuScaleUp { pos, size, draw_state, increment_pos, increment_size, grow_win, .. } => {
                if self.win.is_none() {
                    return;
                }
                let Some(g) = must(wm, *grow_win, "MainMenuScaleUpTransition::draw: no grow window (m_growWin->winGetEnabledImage)") else { return };
                let img = wm.tree.windows[g].inst.enabled_draw_data[0].image.clone();
                let ds = *draw_state;
                if ds <= 0 || ds >= MAINMENU_SCALEUP_END {
                    return;
                }
                let x = pos.0 + increment_pos.0 * ds;
                let y = pos.1 + increment_pos.1 * ds;
                image(wm, out, img.as_ref(), x, y, x + size.0 + increment_size.0 * ds, y + size.1 + increment_size.1 * ds, -1);
            }
            Style::MainMenuGrow { small, pos, size, draw_state, increment_size, .. } => {
                let Some(w) = self.win.and_then(|w| live(wm, w)) else { return };
                let img = wm.tree.windows[w].inst.enabled_draw_data[0].image.clone();
                let ds = *draw_state;
                let end = if *small { MAINMENU_SMALL_END } else { MAINMENU_MEDIUM_END };
                if ds <= 0 || ds >= end {
                    return;
                }
                let x = pos.0 - (increment_size.0 * ds) / 2;
                let y = pos.1 - (increment_size.1 * ds) / 2;
                let x1 = pos.0 + size.0 + (increment_size.0 * ds) / 2;
                let y1 = pos.1 + size.1 + (increment_size.1 * ds) / 2;
                image(wm, out, img.as_ref(), x, y, x1, y1, -1);
            }
            Style::TextType { draw_state, partial_text, .. } => {
                if *draw_state > 0 && *draw_state < self.frame_length {
                    draw_type_text(wm, h, self.win, partial_text, out);
                }
            }
            Style::CountUp { .. } | Style::TextOnFrame | Style::ReverseSound => {}
            Style::ScreenFade { pos, size, percent, draw_state } => {
                let mut alpha = (percent * 255.0f32 * *draw_state as f32) as i32;
                if alpha > 255 {
                    alpha = 255;
                }
                fill(out, pos.0, pos.1, size.0, size.1, game_make_color(0, 0, 0, alpha as u8));
            }
            Style::ControlBarArrow { pos, increment_pos, size, fade_percent, draw_state, arrow, .. } => {
                let ds = *draw_state;
                if ds < 0 {
                    return;
                }
                if ds < CONTROLBAR_BEGIN_FADE {
                    let y = pos.1 + increment_pos.1 * ds;
                    image(wm, out, arrow.as_ref(), pos.0, y, pos.0 + size.0, y + size.1, -1);
                } else {
                    let mut alpha = ((1.0f32 - fade_percent * (ds - CONTROLBAR_BEGIN_FADE) as f32) * 255.0f32) as i32;
                    if alpha > 255 {
                        alpha = 255;
                    }
                    let y = pos.1 + increment_pos.1 * (CONTROLBAR_BEGIN_FADE - 1);
                    image(wm, out, arrow.as_ref(), pos.0, y, pos.0 + size.0, y + size.1, game_make_color(255, 255, 255, alpha as u8));
                }
            }
            Style::FullFade { pos, size, percent, draw_state } => {
                let ds = *draw_state;
                let mut alpha = if ds > FULLFADE_END / 2 { (percent * 255.0f32 * (FULLFADE_END - ds) as f32) as i32 } else { (percent * 255.0f32 * ds as f32) as i32 };
                if alpha > 255 {
                    alpha = 255;
                }
                fill(out, pos.0, pos.1, size.0, size.1, game_make_color(0, 0, 0, alpha as u8));
                open(out, pos.0, pos.1, size.0, size.1, game_make_color(60, 60, 180, alpha as u8));
            }
        }
    }
}

/// `GadgetStaticTextGetText(win)`: empty without a window or user data; a window whose user data is not a static
/// text's is read as one upstream (recorded).
fn static_text(wm: &mut GameWindowManager, w: Option<WinId>) -> Vec<u16> {
    let Some(w) = w else { return Vec::new() };
    let Some(w) = live(wm, w) else { return Vec::new() };
    match &wm.ext[w].user {
        UserData::StaticText(..) | UserData::None | UserData::PushButton(None) => static_text_get_text(wm, w),
        _ => {
            wm.set_undefined("GadgetStaticTextGetText: the window's user data is not a TextData");
            Vec::new()
        }
    }
}

/// `PushButtonImageDrawThree(window, alpha)` (GameWindowTransitionsStyles.cpp's own: the enabled images 0, 5, 6, in
/// the alpha's colour, without text).
pub fn push_button_image_draw_three<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, window: Option<WinId>, alpha: u8, out: &mut Vec<DrawItem>) {
    let Some(w) = must(wm, window, "PushButtonImageDrawThree: no window (window->winGetInstanceData)") else { return };
    let color = game_make_color(255, 255, 255, alpha);
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let (xo, yo) = win.inst.image_offset;
    let d = &win.inst.enabled_draw_data;
    let (Some(left), Some(right), Some(center)) = (d[0].image.clone(), d[6].image.clone(), d[5].image.clone()) else { return };
    let (lw, _) = h.image_size(&left).unwrap_or((0, 0));
    let (rw, _) = h.image_size(&right).unwrap_or((0, 0));
    let (cw, _) = h.image_size(&center).unwrap_or((0, 0));
    let left_end = (ox + lw + xo, oy + sy + yo);
    let right_start = (ox + sx - rw + xo, oy + yo);
    let center_width = right_start.0 - left_end.0;
    if center_width <= 0 {
        let (x0, y0) = (ox + xo, oy + yo);
        let x1 = ox + xo + sx / 2;
        image(wm, out, Some(&left), x0, y0, x1, left_end.1, color);
        let y0 = right_start.1;
        image(wm, out, Some(&right), x1, y0, ox + sx, y0 + sy, color);
    } else {
        if cw == 0 {
            wm.set_undefined("PushButtonImageDrawThree: a centre image of width 0 (centerWidth / 0)");
            return;
        }
        let pieces = center_width / cw;
        let mut x = left_end.0;
        let y = oy + yo;
        let y1 = y + sy + yo;
        for _ in 0..pieces {
            image(wm, out, Some(&center), x, y, x + cw, y1, color);
            x += cw;
        }
        if right_start.0 - x > 0 {
            wm.clip = Some((x, y, right_start.0, y1));
            image(wm, out, Some(&center), x, y, x + cw, y1, color);
            wm.clip = None;
        }
        image(wm, out, Some(&left), ox + xo, oy + yo, left_end.0, left_end.1, color);
        image(wm, out, Some(&right), right_start.0, right_start.1, right_start.0 + rw, right_start.1 + sy, color);
    }
}

/// `drawTypeText(window, str)`: the partial text, placed by the window's whole text's size.
fn draw_type_text<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, window: Option<WinId>, partial: &[u16], out: &mut Vec<DrawItem>) {
    let Some(w) = must(wm, window, "drawTypeText: no window") else { return };
    let UserData::StaticText(td, text) = wm.ext[w].user.clone() else {
        wm.set_undefined("drawTypeText: the window's user data is not a TextData");
        return;
    };
    if text.is_empty() {
        return;
    }
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let wrap = sx - 10;
    let (tw, th) = h.text_size(win.inst.font.as_ref(), &text, Some(wrap));
    let x = if td.centered { ox + sx / 2 - tw / 2 } else { ox + 7 };
    let y = oy + sy / 2 - th / 2;
    out.push(DrawItem::Text {
        text: partial.to_vec(),
        x,
        y,
        color: win.inst.enabled_text.color,
        drop_color: win.inst.enabled_text.border_color,
        font: win.inst.font.clone(),
        word_wrap: Some(wrap),
        wrap_centered: win.status & win_status::WRAP_CENTERED != 0,
        hotkey: false,
        clip: Some((ox, oy, ox + sx, oy + sy)),
    });
}

// ---------------------------------------------------------------------------
// TransitionWindow, TransitionGroup
// ---------------------------------------------------------------------------

impl TransitionWindow {
    /// `init()`: the window found by its name, a new transition of the style inited on it.
    pub fn init<H: GuiHost>(&mut self, wm: &mut GameWindowManager, h: &mut H) {
        self.win = find_window(wm, &self.win_name);
        self.current_frame_delay = self.frame_delay;
        let Some(mut t) = get_transition_for_style(self.style) else {
            wm.set_undefined("TransitionWindow::init: an unknown style (getTransitionForStyle returns NULL)");
            self.transition = None;
            return;
        };
        t.init(wm, h, self.win);
        self.transition = Some(t);
    }
    /// `update(frame)`.
    pub fn update<H: GuiHost>(&mut self, wm: &mut GameWindowManager, h: &mut H, frame: i32) {
        let Some(t) = self.transition.as_mut() else {
            wm.set_undefined("TransitionWindow::update: no transition (m_transition->getFrameLength)");
            return;
        };
        if frame < self.current_frame_delay || frame > self.current_frame_delay.wrapping_add(t.frame_length) {
            return;
        }
        t.update(wm, h, frame - self.current_frame_delay);
    }
    /// `isFinished()`.
    pub fn is_finished(&self) -> bool {
        self.transition.as_ref().is_none_or(|t| t.is_finished)
    }
    /// `reverse(totalFrames)` (the total is not used).
    pub fn reverse(&mut self, wm: &mut GameWindowManager) {
        if let Some(t) = self.transition.as_mut() {
            t.reverse(wm);
        }
    }
    /// `skip()`.
    pub fn skip<H: GuiHost>(&mut self, wm: &mut GameWindowManager, h: &mut H) {
        if let Some(t) = self.transition.as_mut() {
            t.skip(wm, h);
        }
    }
    /// `draw()`.
    pub fn draw<H: GuiHost>(&self, wm: &mut GameWindowManager, h: &mut H, out: &mut Vec<DrawItem>) {
        if let Some(t) = self.transition.as_ref() {
            t.draw(wm, h, out);
        }
    }
    /// `getTotalFrames()`.
    pub fn get_total_frames(&self) -> i32 {
        match &self.transition {
            Some(t) => self.frame_delay.wrapping_add(t.frame_length),
            None => self.frame_delay,
        }
    }
}

impl TransitionGroup {
    /// `init()`.
    pub fn init<H: GuiHost>(&mut self, wm: &mut GameWindowManager, h: &mut H) {
        self.current_frame = 0;
        self.direction_multiplier = 1;
        for t in &mut self.windows {
            t.init(wm, h);
        }
    }
    /// `update()`.
    pub fn update<H: GuiHost>(&mut self, wm: &mut GameWindowManager, h: &mut H) {
        self.current_frame = self.current_frame.wrapping_add(self.direction_multiplier);
        for t in &mut self.windows {
            t.update(wm, h, self.current_frame);
        }
    }
    /// `isFinished()`.
    pub fn is_finished(&self) -> bool {
        self.windows.iter().all(|t| t.is_finished())
    }
    /// `reverse()`.
    pub fn reverse(&mut self, wm: &mut GameWindowManager) {
        self.direction_multiplier = -1;
        let total = self.windows.iter().map(|t| t.get_total_frames()).fold(0, i32::max);
        for t in &mut self.windows {
            t.reverse(wm);
        }
        self.current_frame = total;
    }
    /// `isReversed()`.
    pub fn is_reversed(&self) -> bool {
        self.direction_multiplier < 0
    }
    /// `skip()`.
    pub fn skip<H: GuiHost>(&mut self, wm: &mut GameWindowManager, h: &mut H) {
        for t in &mut self.windows {
            t.skip(wm, h);
        }
    }
    /// `draw()`.
    pub fn draw<H: GuiHost>(&self, wm: &mut GameWindowManager, h: &mut H, out: &mut Vec<DrawItem>) {
        for t in &self.windows {
            t.draw(wm, h, out);
        }
    }
}

// ---------------------------------------------------------------------------
// the handler
// ---------------------------------------------------------------------------

impl GameWindowTransitionsHandler {
    /// `init()` and `reset()`: no current, pending or drawn group.
    pub fn reset(&mut self) {
        self.current = None;
        self.pending = None;
        self.draw_group = None;
        self.secondary_draw_group = None;
    }

    /// `findGroup(name)`: without case; none for an empty name.
    pub fn find_group(&self, name: &[u8]) -> Option<usize> {
        if name.is_empty() {
            return None;
        }
        self.groups.iter().position(|g| g.name.eq_ignore_ascii_case(name))
    }

    /// `getNewGroup(name)`: none for an empty name or one already there.
    pub fn get_new_group(&mut self, name: &[u8]) -> Option<usize> {
        if name.is_empty() || self.find_group(name).is_some() {
            return None;
        }
        self.groups.push(TransitionGroup { name: name.to_vec(), ..Default::default() });
        Some(self.groups.len() - 1)
    }

    /// `isFinished()`.
    pub fn is_finished(&self) -> bool {
        self.current.is_none_or(|c| self.groups[c].is_finished())
    }

    /// `INI::load` of `WindowTransitions.ini`'s text (`load()`).
    pub fn load_text(&mut self, path: &[u8], text: &[u8]) -> IniResult<()> {
        Ini::load(path, text, IniLoadType::Overwrite, None, WINDOW_TRANSITION_BLOCKS, self)
    }
}

/// The target `initFromINI` fills: the new group, or upstream's `NULL` for a name already there (`None`).
struct GroupTarget {
    group: Option<TransitionGroup>,
    undefined: Option<&'static str>,
}

fn null_group(t: &mut GroupTarget) -> Option<&mut TransitionGroup> {
    if t.group.is_none() && t.undefined.is_none() {
        t.undefined = Some("INI::parseWindowTransitions: a field parsed into the NULL group of a name already there");
    }
    t.group.as_mut()
}

/// `GameWindowTransitionsHandler::parseWindow`: a `Window` block, added to the group.
fn parse_window(ini: &mut Ini<'_>, t: &mut GroupTarget, _: &IniUserData) -> IniResult<()> {
    let mut w = TransitionWindow::default();
    ini.init_from_ini(&mut w, WINDOW_FIELDS)?;
    if let Some(g) = null_group(t) {
        g.windows.push(w);
    }
    Ok(())
}

/// `parseWindow`'s `myFieldParse`.
const WINDOW_FIELDS: &[FieldParse<TransitionWindow>] = &[
    FieldParse::new("WinName", |ini, w, _| ini.parse_ascii_string().map(|v| w.win_name = v), IniUserData::None),
    FieldParse::new("Style", |ini, w, ud| ini.parse_lookup_list(ud.lookup()).map(|v| w.style = v), IniUserData::Lookup(TRANSITION_STYLE_NAMES)),
    FieldParse::new("FrameDelay", |ini, w, _| ini.parse_int().map(|v| w.frame_delay = v), IniUserData::None),
    FieldParse::end(),
];

/// `m_gameWindowTransitionsFieldParseTable`.
const GROUP_FIELDS: &[FieldParse<GroupTarget>] = &[
    FieldParse::new("Window", parse_window, IniUserData::None),
    FieldParse::new(
        "FireOnce",
        |ini, t, _| {
            let v = ini.parse_bool()?;
            if let Some(g) = null_group(t) {
                g.fire_once = v;
            }
            Ok(())
        },
        IniUserData::None,
    ),
    FieldParse::end(),
];

/// `INI::parseWindowTransitions`: the `WindowTransition` block. The group is filled apart and put in its place after
/// (nothing else reads it while its block is parsed).
fn parse_window_transitions(ini: &mut Ini<'_>, handler: &mut GameWindowTransitionsHandler) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    let index = handler.get_new_group(&name);
    let mut t = GroupTarget { group: index.map(|i| std::mem::take(&mut handler.groups[i])), undefined: None };
    let r = ini.init_from_ini(&mut t, GROUP_FIELDS);
    if let (Some(i), Some(g)) = (index, t.group) {
        handler.groups[i] = g;
    }
    if handler.undefined.is_none() {
        handler.undefined = t.undefined;
    }
    r
}

/// The block table of `WindowTransitions.ini`.
pub const WINDOW_TRANSITION_BLOCKS: &[BlockParse<GameWindowTransitionsHandler>] = &[BlockParse { token: "WindowTransition", parse: parse_window_transitions }];

/// `GameWindowTransitionsHandler::load()` on the file's text: the groups added; upstream's undefined behaviour in the
/// parse is recorded in the manager.
pub fn load_window_transitions(wm: &mut GameWindowManager, path: &[u8], text: &[u8]) -> IniResult<()> {
    let r = wm.transitions.load_text(path, text);
    if let Some(u) = wm.transitions.undefined {
        wm.set_undefined(u);
    }
    r
}

/// The handler taken out of the manager for `f` (so that the styles can change the windows), and put back.
fn with<R>(wm: &mut GameWindowManager, f: impl FnOnce(&mut GameWindowTransitionsHandler, &mut GameWindowManager) -> R) -> R {
    let mut th = std::mem::take(&mut wm.transitions);
    let r = f(&mut th, wm);
    wm.transitions = th;
    r
}

/// `TheTransitionHandler->update()`.
pub fn update<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H) {
    with(wm, |th, wm| {
        th.secondary_draw_group = if th.draw_group != th.current { th.draw_group } else { None };
        th.draw_group = th.current;
        if let Some(c) = th.current {
            if !th.groups[c].is_finished() {
                th.groups[c].update(wm, h);
            }
        }
        if let Some(c) = th.current {
            if th.groups[c].is_finished() && th.groups[c].fire_once && !crate::logic::modules::plant(4712) {
                th.current = None;
            }
        }
        if let (Some(c), Some(_)) = (th.current, th.pending) {
            if th.groups[c].is_finished() {
                th.current = th.pending.take();
            }
        }
        if th.current.is_none() && th.pending.is_some() {
            th.current = th.pending.take();
        }
        if let Some(c) = th.current {
            if th.groups[c].is_finished() && th.groups[c].is_reversed() {
                th.current = None;
            }
        }
    })
}

/// `TheTransitionHandler->draw()`: this frame's group, then the last frame's once more.
pub fn draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, out: &mut Vec<DrawItem>) {
    with(wm, |th, wm| {
        if let Some(d) = th.draw_group {
            th.groups[d].draw(wm, h, out);
        }
        if let Some(d) = th.secondary_draw_group {
            th.groups[d].draw(wm, h, out);
        }
    })
}

/// `TheTransitionHandler->setGroup(groupName, immidiate)`.
pub fn set_group<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, name: &[u8], immediate: bool) {
    with(wm, |th, wm| {
        if name.is_empty() && immediate {
            th.current = None;
        }
        if immediate {
            if let Some(c) = th.current {
                th.groups[c].skip(wm, h);
                th.current = th.find_group(name);
                if let Some(c) = th.current {
                    th.groups[c].init(wm, h);
                }
                return;
            }
        }
        if let Some(c) = th.current {
            if !th.groups[c].fire_once && !th.groups[c].is_reversed() {
                th.groups[c].reverse(wm);
            }
            th.pending = th.find_group(name);
            if let Some(p) = th.pending {
                th.groups[p].init(wm, h);
            }
            return;
        }
        th.current = th.find_group(name);
        if let Some(c) = th.current {
            th.groups[c].init(wm, h);
        }
    })
}

/// `TheTransitionHandler->reverse(groupName)`.
pub fn reverse<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, name: &[u8]) {
    with(wm, |th, wm| {
        let g = th.find_group(name);
        if th.current == g {
            match th.current {
                Some(c) => th.groups[c].reverse(wm),
                None => wm.set_undefined("GameWindowTransitionsHandler::reverse: an unknown group with no current one (NULL->reverse)"),
            }
            return;
        }
        if th.pending == g {
            th.pending = None;
            return;
        }
        if let Some(c) = th.current {
            th.groups[c].skip(wm, h);
        }
        if let Some(p) = th.pending {
            th.groups[p].skip(wm, h);
        }
        th.current = g;
        let Some(c) = g else {
            wm.set_undefined("GameWindowTransitionsHandler::reverse: an unknown group (NULL->init)");
            return;
        };
        th.groups[c].init(wm, h);
        th.groups[c].skip(wm, h);
        th.groups[c].reverse(wm);
        th.pending = None;
    })
}

/// `TheTransitionHandler->remove(groupName, skipPending)`.
pub fn remove<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, name: &[u8], skip_pending: bool) {
    with(wm, |th, wm| {
        let g = th.find_group(name);
        if th.pending == g {
            if skip_pending {
                match th.pending {
                    Some(p) => th.groups[p].skip(wm, h),
                    None => {
                        wm.set_undefined("GameWindowTransitionsHandler::remove: an unknown group, skipped as the NULL pending one");
                        return;
                    }
                }
            }
            th.pending = None;
        }
        if th.current == g {
            match th.current {
                Some(c) => th.groups[c].skip(wm, h),
                None => {
                    wm.set_undefined("GameWindowTransitionsHandler::remove: an unknown group with no current one (NULL->skip)");
                    return;
                }
            }
            th.current = None;
            if th.pending.is_some() {
                th.current = th.pending;
            }
        }
    })
}

/// `TheTransitionHandler->isFinished()`.
pub fn is_finished(wm: &GameWindowManager) -> bool {
    wm.transitions.is_finished()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::window_manager::SystemFunc;
    use crate::client::window_script::{gws, GadgetData, TextData, WinInstanceData};

    /// A host that records the sounds, has `Img*` images (5x10, `*Mid` 7x10) and `Gradient`, and the arrow image.
    #[derive(Default)]
    pub(crate) struct Rec {
        pub audio: Vec<Vec<u8>>,
        pub arrow: Option<Vec<u8>>,
    }
    impl GuiHost for Rec {
        fn add_audio_event(&mut self, name: &[u8]) {
            self.audio.push(name.to_vec());
        }
        fn image_size(&self, image: &[u8]) -> Option<(i32, i32)> {
            Some(if image.ends_with(b"Mid") { (7, 10) } else { (5, 10) })
        }
        fn has_image(&self, image: &[u8]) -> bool {
            image.starts_with(b"Img") || image == b"Gradient"
        }
        fn control_bar_arrow_image(&self) -> Option<Vec<u8>> {
            self.arrow.clone()
        }
    }

    /// A window named `name` at (x, y), w x h, shown: a user window, or a static text with `text`.
    pub(crate) fn named(wm: &mut GameWindowManager, h: &mut Rec, name: &str, (x, y, w, hh): (i32, i32, i32, i32), text: Option<&str>) -> WinId {
        let id = wm.tree.windows.len() as i32 + 100;
        let style = if text.is_some() { gws::STATIC_TEXT } else { gws::USER_WINDOW };
        let inst = WinInstanceData { style, id, decorated_name_string: name.as_bytes().to_vec(), ..WinInstanceData::new() };
        let win = match text {
            Some(t) => {
                let w = wm.gogo_gadget_look(h, None, win_status::ENABLED, x, y, w, hh, inst, GadgetData::Text(TextData::default()), false).expect("a static text");
                static_text_set_text(wm, h, Some(w), &t.encode_utf16().collect::<Vec<u16>>());
                w
            }
            None => wm.win_create(h, None, win_status::ENABLED, x, y, w, hh, SystemFunc::Default, Some(inst)),
        };
        wm.tree.windows[win].id = id;
        win
    }

    /// The handler's draws alone (`TheTransitionHandler->draw()`).
    fn drawn(wm: &mut GameWindowManager, h: &mut Rec) -> Vec<DrawItem> {
        let mut v = Vec::new();
        draw(wm, h, &mut v);
        v
    }

    fn hidden(wm: &GameWindowManager, w: WinId) -> bool {
        wm.tree.windows[w].is_hidden()
    }

    fn ini(groups: &str) -> GameWindowManager {
        let mut wm = GameWindowManager::new((800, 600));
        load_window_transitions(&mut wm, WINDOW_TRANSITIONS_INI, groups.as_bytes()).expect("reads");
        wm
    }

    /// The INI, predicted from GameWindowTransitions.cpp: a group per `WindowTransition` block, its windows in order
    /// with their name, style (any case) and delay (0 and `FLASH` when absent), `FireOnce`; a name already there
    /// (without case) gives a `NULL` group: an empty block is harmless, a field in it is undefined.
    #[test]
    fn the_ini_makes_the_groups_as_upstream() {
        let wm = ini("; c\nWindowTransition One\n  FireOnce = Yes\n  Window\n    WinName = A\n    Style = winfade\n    FrameDelay = 3\n  End\n  Window\n    WinName = B\n  End\nEnd\nWindowTransition ONE\nEnd\n");
        assert!(wm.undefined.is_none());
        let t = &wm.transitions;
        assert_eq!(t.groups.len(), 1);
        let g = &t.groups[0];
        assert!(g.fire_once && g.name == b"One");
        assert_eq!((g.windows[0].win_name.as_slice(), g.windows[0].style, g.windows[0].frame_delay), (&b"A"[..], style::WIN_FADE_TRANSITION, 3));
        assert_eq!((g.windows[1].style, g.windows[1].frame_delay), (style::TRANSITION_FLASH, 0));
        assert_eq!(t.find_group(b"one"), Some(0));
        assert_eq!(t.find_group(b""), None);
        let wm = ini("WindowTransition One\nEnd\nWindowTransition one\n  FireOnce = No\nEnd\n");
        assert!(wm.undefined.is_some());
    }

    /// A `FLASH` window, predicted from FlashTransition: `setGroup` inits it (frame 0 backwards hides the window);
    /// frames 1 to 3 keep it hidden and draw the white box (frame 1 plays `GUIBoarderFadeIn`), 4 to 7 show it, 8
    /// finishes; the finished fire-once group is dropped by the update after. Reversed (a `reverse` of the current
    /// group) it runs back from 8 and frame 0 hides it again; finished and reversed, it is dropped.
    #[test]
    fn a_flash_group_runs_forward_and_back() {
        let mut wm = ini("WindowTransition G\n  Window\n    WinName = W\n    Style = FLASH\n  End\nEnd\n");
        let mut h = Rec::default();
        let w = named(&mut wm, &mut h, "W", (10, 20, 100, 50), None);
        set_group(&mut wm, &mut h, b"G", false);
        assert!(hidden(&wm, w));
        assert!(!is_finished(&wm));
        wm.update(&mut h);
        assert_eq!(h.audio, [b"GUIBoarderFadeIn".to_vec()]);
        let draws = wm.win_repaint(&mut h);
        assert_eq!(draws, [DrawItem::OpenRect { x: 11, y: 21, w: 98, h: 50, color: game_make_color(255, 255, 255, 100) }, DrawItem::FillRect { x: 11, y: 21, w: 98, h: 50, color: game_make_color(255, 255, 255, 33) }]);
        for _ in 0..3 {
            wm.update(&mut h);
        }
        assert!(!hidden(&wm, w), "frame 4 shows it");
        for _ in 0..4 {
            wm.update(&mut h);
        }
        assert!(is_finished(&wm) && !hidden(&wm, w));
        reverse(&mut wm, &mut h, b"G");
        assert!(!is_finished(&wm));
        for _ in 0..8 {
            wm.update(&mut h);
        }
        assert!(hidden(&wm, w) && wm.transitions.current.is_none(), "finished and reversed: dropped");
        // a fire-once group is dropped once finished
        let mut wm = ini("WindowTransition G\n  FireOnce = Yes\n  Window\n    WinName = W\n    Style = REVERSESOUND\n  End\nEnd\n");
        set_group(&mut wm, &mut h, b"G", false);
        assert!(is_finished(&wm));
        wm.update(&mut h);
        assert!(wm.transitions.current.is_none());
    }

    /// The handler, predicted from GameWindowTransitionsHandler: `setGroup` with a group running reverses it and
    /// makes the new one pending, which takes over once the first is finished; `remove` of the current group skips it
    /// and gives the place to the pending one; immediate `setGroup` skips the current one; `reverse` of the pending
    /// group drops it; `reverse` (and `remove`) of an unknown group with none current is undefined. The window a group
    /// names but the screen lacks never finishes (`FLASH` needs a window).
    #[test]
    fn the_handler_orders_the_groups_as_upstream() {
        let mut wm = ini("WindowTransition A\n  Window\n    WinName = X\n    Style = FLASH\n  End\nEnd\nWindowTransition B\n  Window\n    WinName = Y\n    Style = FLASH\n  End\nEnd\nWindowTransition C\n  Window\n    WinName = Missing\n  End\nEnd\n");
        let mut h = Rec::default();
        let x = named(&mut wm, &mut h, "X", (0, 0, 10, 10), None);
        let y = named(&mut wm, &mut h, "Y", (0, 0, 10, 10), None);
        set_group(&mut wm, &mut h, b"A", false);
        for _ in 0..8 {
            wm.update(&mut h);
        }
        assert!(!hidden(&wm, x));
        set_group(&mut wm, &mut h, b"B", false);
        assert_eq!((wm.transitions.current, wm.transitions.pending), (Some(0), Some(1)));
        assert!(wm.transitions.groups[0].is_reversed() && hidden(&wm, y));
        for _ in 0..8 {
            wm.update(&mut h);
        }
        assert_eq!(wm.transitions.current, Some(1), "A finished backwards: B takes over");
        assert!(hidden(&wm, x));
        assert_eq!(wm.transitions.secondary_draw_group, None);
        set_group(&mut wm, &mut h, b"A", true);
        assert!(!hidden(&wm, y), "immediate: B skipped to its end");
        assert_eq!(wm.transitions.current, Some(0));
        set_group(&mut wm, &mut h, b"B", false);
        reverse(&mut wm, &mut h, b"B");
        assert_eq!(wm.transitions.pending, None);
        remove(&mut wm, &mut h, b"A", false);
        assert_eq!(wm.transitions.current, None);
        set_group(&mut wm, &mut h, b"C", false);
        for _ in 0..20 {
            wm.update(&mut h);
        }
        assert!(!is_finished(&wm), "a missing window never finishes");
        wm.transitions.reset();
        reverse(&mut wm, &mut h, b"Nowhere");
        assert!(wm.undefined.is_some());
        let mut wm = ini("");
        remove(&mut wm, &mut h, b"Nowhere", true);
        assert!(wm.undefined.is_some());
    }

    /// Every style on a window that has what it needs, forward to its end, drawn each frame, then reversed to frame
    /// 0 and skipped: no undefined behaviour, each finished both ways (but `TYPETEXT` shorter than 30, which only a
    /// skip finishes), and the draws each style makes (predicted from the styles' draw functions).
    #[test]
    fn every_style_runs_both_ways() {
        let mut groups = String::new();
        for (i, s) in TRANSITION_STYLE_NAMES.iter().enumerate() {
            let win = match s.value {
                style::TEXT_TYPE_TRANSITION | style::COUNT_UP_TRANSITION => "T",
                _ => "W",
            };
            groups += &format!("WindowTransition S{i}\n  Window\n    WinName = {win}\n    Style = {}\n    FrameDelay = 1\n  End\nEnd\n", s.name);
        }
        let mut wm = ini(&groups);
        let mut h = Rec { arrow: Some(b"ImgArrow".to_vec()), ..Default::default() };
        let w = named(&mut wm, &mut h, "W", (100, 100, 60, 40), None);
        for (k, img) in [(0, "ImgL"), (5, "ImgMid"), (6, "ImgR")] {
            wm.tree.windows[w].inst.enabled_draw_data[k].image = Some(img.as_bytes().to_vec());
        }
        wm.tree.windows[w].inst.disabled_draw_data[0].image = Some(b"ImgD".to_vec());
        let grow = [named(&mut wm, &mut h, "WMedium", (90, 90, 80, 60), None), named(&mut wm, &mut h, "WSmall", (110, 110, 30, 20), None), named(&mut wm, &mut h, "MainMenu.wnd:WinGrowMarker", (0, 0, 200, 100), None)];
        named(&mut wm, &mut h, "ControlBar.wnd:ButtonGeneral", (400, 500, 40, 40), None);
        let t = named(&mut wm, &mut h, "T", (10, 10, 200, 30), Some("1234"));
        let mut kinds = Vec::new();
        for (i, s) in TRANSITION_STYLE_NAMES.iter().enumerate() {
            for &g in &grow {
                wm.win_hide(g, false);
            }
            wm.win_hide(w, false);
            wm.win_hide(t, false);
            let name = format!("S{i}");
            set_group(&mut wm, &mut h, name.as_bytes(), true);
            let mut drawn = Vec::new();
            for _ in 0..40 {
                wm.update(&mut h);
                draw(&mut wm, &mut h, &mut drawn);
                assert!(wm.undefined.is_none(), "{}: {:?}", s.name, wm.undefined);
            }
            let short_text = s.value == style::TEXT_TYPE_TRANSITION;
            assert_eq!(is_finished(&wm), !short_text, "{} forward", s.name);
            reverse(&mut wm, &mut h, name.as_bytes());
            for _ in 0..40 {
                wm.update(&mut h);
                draw(&mut wm, &mut h, &mut drawn);
            }
            assert!(wm.undefined.is_none(), "{}: {:?}", s.name, wm.undefined);
            assert!(wm.transitions.current.is_none() || short_text, "{} backward", s.name);
            set_group(&mut wm, &mut h, name.as_bytes(), true);
            set_group(&mut wm, &mut h, b"", true);
            assert!(wm.undefined.is_none(), "{}: {:?}", s.name, wm.undefined);
            kinds.push((s.name, drawn.len()));
        }
        // the styles that draw nothing: COUNTUP, TEXTONFRAME, REVERSESOUND
        for (n, d) in &kinds {
            assert_eq!(*d == 0, ["COUNTUP", "TEXTONFRAME", "REVERSESOUND"].contains(n), "{n} draws {d}");
        }
        // COUNTUP, reversed to its frame 0, set "0" (and hid its window, so the next init finished at once: predicted
        // "1234", the forward end; missed); MAINMENUSCALEUP gave the grow window W's disabled image
        assert_eq!(crate::client::gadget::static_text_get_text(&wm, t), "0".encode_utf16().collect::<Vec<u16>>());
        assert_eq!(wm.tree.windows[grow[2]].inst.enabled_draw_data[0].image.as_deref(), Some(&b"ImgD"[..]));
        assert_eq!(wm.tree.windows[grow[1]].inst.enabled_draw_data[0].image.as_deref(), Some(&b"ImgL"[..]), "the small scale gives its own enabled image");
        for snd in ["GUIBoarderFadeIn", "GUIButtonsFadeIn", "GUILogoMouseOver", "GUILogoSelect", "GUITypeText", "GUIScoreScreenTick", "GUIScoreScreenPictures", "GUITransitionFade"] {
            assert!(h.audio.contains(&snd.as_bytes().to_vec()), "{snd}");
        }
    }

    /// The numbers, predicted from CountUpTransition and the styles' draws: `250` counts by hundreds over 2 frames
    /// (`100`, then the whole text); `4000` by thousands; `ScreenFade`'s alpha at frame 29 is `(1/29) * 255 * 29`
    /// truncated (255); `FullFade` at frame 5 is `0.2 * 255 * 5` (255); the arrow falls from `20 - height` by
    /// `y / 16` a frame and fades from frame 16.
    #[test]
    fn the_counts_alphas_and_arrow() {
        let mut wm = ini("WindowTransition C\n  Window\n    WinName = T\n    Style = COUNTUP\n  End\nEnd\nWindowTransition F\n  Window\n    WinName = T\n    Style = SCREENFADE\n  End\nEnd\nWindowTransition A\n  Window\n    Style = CONTROLBARARROW\n  End\nEnd\n");
        let mut h = Rec { arrow: Some(b"ImgArrow".to_vec()), ..Default::default() };
        let t = named(&mut wm, &mut h, "T", (0, 0, 50, 20), Some("250"));
        named(&mut wm, &mut h, "ControlBar.wnd:ButtonGeneral", (400, 480, 40, 40), None);
        set_group(&mut wm, &mut h, b"C", false);
        let text = |wm: &GameWindowManager| String::from_utf16_lossy(&crate::client::gadget::static_text_get_text(wm, t));
        assert_eq!(text(&wm), "0");
        wm.update(&mut h);
        assert_eq!(text(&wm), "100");
        wm.update(&mut h);
        assert_eq!(text(&wm), "250");
        assert!(is_finished(&wm));
        set_group(&mut wm, &mut h, b"F", true);
        for _ in 0..29 {
            wm.update(&mut h);
        }
        assert_eq!(drawn(&mut wm, &mut h), [DrawItem::FillRect { x: 0, y: 0, w: 800, h: 600, color: game_make_color(0, 0, 0, 255) }]);
        set_group(&mut wm, &mut h, b"A", true);
        wm.update(&mut h);
        // (this frame's group, then the last frame's once more: the screen fade skipped to frame 30, 263 clamped)
        assert_eq!(
            drawn(&mut wm, &mut h),
            [
                DrawItem::Image { image: b"ImgArrow".to_vec(), x0: 418, y0: 10 + 30, x1: 423, y1: 50, color: u32::MAX, grayscale: false, clip: None },
                DrawItem::FillRect { x: 0, y: 0, w: 800, h: 600, color: game_make_color(0, 0, 0, 255) }
            ]
        );
        assert_eq!(atoi(&"  -12x".encode_utf16().collect::<Vec<u16>>()), Some(-12));
        assert_eq!(atoi(&"99999999999".encode_utf16().collect::<Vec<u16>>()), None);
        assert_eq!(format_int(-7), "-7".encode_utf16().collect::<Vec<u16>>());
        assert_eq!(get_transition_for_style(15), None);
    }

    /// Undefined behaviour is recorded, predicted from the styles: a `WINFADE` frame without its window, a
    /// `MAINMENUMEDIUMSCALEUP` without a window, a transition holding a window freed by `processDestroyList`,
    /// `TYPETEXT` drawn on a window that is not a static text, `drawTypeText`'s text placed by the whole text.
    #[test]
    fn upstreams_undefined_behaviour_is_recorded() {
        let mut h = Rec::default();
        for (style, setup) in [("WINFADE", 0), ("MAINMENUMEDIUMSCALEUP", 0), ("FLASH", 1)] {
            let mut wm = ini(&format!("WindowTransition G\n  Window\n    WinName = W\n    Style = {style}\n  End\nEnd\n"));
            if setup == 1 {
                let w = named(&mut wm, &mut h, "W", (0, 0, 10, 10), None);
                set_group(&mut wm, &mut h, b"G", false);
                wm.win_destroy(Some(w));
                wm.update(&mut h);
            } else {
                set_group(&mut wm, &mut h, b"G", false);
                wm.update(&mut h);
            }
            assert!(wm.undefined.is_some(), "{style}");
        }
        let mut wm = ini("WindowTransition G\n  Window\n    WinName = T\n    Style = TYPETEXT\n  End\nEnd\n");
        let t = named(&mut wm, &mut h, "T", (10, 10, 100, 30), Some("ABC"));
        wm.ext[t].user = match wm.ext[t].user.clone() {
            UserData::StaticText(mut td, s) => {
                td.centered = true;
                UserData::StaticText(td, s)
            }
            u => u,
        };
        set_group(&mut wm, &mut h, b"G", false);
        wm.update(&mut h);
        let d = drawn(&mut wm, &mut h);
        let Some(DrawItem::Text { text, x, y, clip, .. }) = d.last() else { panic!("{d:?}") };
        assert_eq!(text, &[65u16]);
        // the whole text "ABC" in the static text's default font (14 points: predicted without a font, 12; missed) is
        // 3 * 7 wide, 14 high (the default measure)
        assert_eq!((*x, *y, *clip), (10 + 50 - 10, 10 + 15 - 7, Some((10, 10, 110, 40))));
        wm.ext[t].user = UserData::None;
        let _ = drawn(&mut wm, &mut h);
        assert!(wm.undefined.is_some());
    }
}
