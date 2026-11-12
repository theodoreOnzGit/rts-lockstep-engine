// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (InGameUI::popupMessage, both forms, and
//     clearPopupMessageData, lines 5161-5226)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/InGamePopupMessage.cpp (the statics,
//     InGamePopupMessageInit, InGamePopupMessageInput, InGamePopupMessageSystem)
//   GeneralsMD/Code/GameEngine/Include/GameClient/InGameUI.h (PopupMessageData)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogicDispatch.cpp (MSG_CLEAR_INGAME_POPUP_MESSAGE, line 896)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #151: the popup message, the boxed text a script shows (`POPUP_MESSAGE`, the logic's
//! `ClientEvent::InGameUi(PopupMessage)`), with its OK button.
//!
//! # As upstream does it
//!
//! - **`popupMessage(identifier, x, y, width, [textColor,] pause, pauseMusic)`** (InGameUI.cpp:5169,
//!   [`popup_message`]): any current message is cleared, the label goes to the briefing history
//!   (`UpdateDiplomacyBriefingText(identifier, FALSE)`), and `m_popupMessageData` holds the text
//!   (`TheGameText->fetch`), the place (x and y percentages of the display, each clamped to 0..100), the width (at least
//!   50), the colour (`PopupMessageColor` for the form without one), and the pause. A pausing message pauses the game
//!   (`TheGameLogic->setGamePaused(TRUE, pauseMusic)`). Then `InGamePopupMessage.wnd` is made and its init run.
//! - **`InGamePopupMessageInit`** ([`in_game_popup_message_init`]) finds the parent, `StaticTextMessage` and `ButtonOk`,
//!   measures the text in the static text's font word-wrapped at `width - 14`, and lays the box out: the parent at
//!   (x, y), `width` wide and `height + 7 + 2 + 2 + okHeight + 2` high; the text at (2, 2), `width - 4` by `height + 7`;
//!   the button at (`width - okWidth - 2`, `height + 11`); the text in the colour (border 0). A pausing message is
//!   modal. The parent takes the focus, is shown and brought to the top.
//! - **`InGamePopupMessageInput`** ([`in_game_popup_message_input`]): Enter or Escape, when released, is a click on OK
//!   (`GBM_SELECTED` sent to the window); either key is eaten.
//! - **`InGamePopupMessageSystem`** ([`in_game_popup_message_system`]): the window wants the focus; OK of a pausing
//!   message clears it at once (`clearPopupMessageData`), of another sends `MSG_CLEAR_INGAME_POPUP_MESSAGE`, which the
//!   logic gives back to every machine's `InGameUI` (GameLogicDispatch.cpp:896, `ClientCall::ClearPopupMessage`).
//! - **`clearPopupMessageData`** ([`clear_popup_message_data`]) destroys the layout, unpauses a pausing message's game
//!   (`setGamePaused(FALSE, pauseMusic)`) and frees the data. `InGameUI::reset` calls it.
//!
//! # Where this differs from upstream
//!
//! - The calls on the window manager, the text, the game and the message stream are the host's ([`PopupMessageHost`]):
//!   the client's is the shell's window manager.
//! - Upstream dereferences the three windows and the layout without checking: a `InGamePopupMessage.wnd` without them
//!   crashes. Here the call that would is skipped and [`InGamePopupMessage::undefined`] records it.
//! - (#155) A client with no window manager (no shell yet, or none) holds the message without a window
//!   ([`WindowlessPopup`]: upstream's data, history and pause, made as upstream makes them) and shows it in the shell when
//!   one comes ([`show_held_popup_message`]). New: upstream always has its window manager.

use crate::client::window_manager::MsgHandled;
use crate::client::window_script::{GameFont, WinId};
use crate::logic::modules::plant;

/// `class PopupMessageData` (InGameUI.h:225).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopupMessageData {
    pub message: Vec<u16>,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub text_color: i32,
    pub pause: bool,
    pub pause_music: bool,
    /// `layout` (`None` for `NULL`).
    pub layout: Option<usize>,
}

/// `InGameUI::m_popupMessageData` and InGamePopupMessage.cpp's statics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InGamePopupMessage {
    /// `m_popupMessageData` (`None` for `NULL`).
    pub popup_message_data: Option<PopupMessageData>,
    /// `parentID`, `staticTextMessageID`, `buttonOkID` (`NAMEKEY_INVALID` 0 before the first init).
    pub parent_id: i32,
    pub static_text_message_id: i32,
    pub button_ok_id: i32,
    /// `parent`, `staticTextMessage`, `buttonOk`.
    pub parent: Option<WinId>,
    pub static_text_message: Option<WinId>,
    pub button_ok: Option<WinId>,
    /// `pause`.
    pub pause: bool,
    /// Upstream's undefined behaviour met (the first): a `NULL` window or layout dereferenced.
    pub undefined: Option<&'static str>,
}

impl InGamePopupMessage {
    fn set_undefined(&mut self, what: &'static str) {
        if self.undefined.is_none() {
            self.undefined = Some(what);
        }
    }
}

/// What the popup's code calls, each named as upstream's call.
pub trait PopupMessageHost {
    /// `TheInGameUI`'s popup and InGamePopupMessage.cpp's statics.
    fn popup(&mut self) -> &mut InGamePopupMessage;
    /// `TheGameText->fetch(label)`.
    fn fetch(&mut self, label: &[u8]) -> Vec<u16>;
    /// `UpdateDiplomacyBriefingText(label, FALSE)`.
    fn update_diplomacy_briefing_text(&mut self, label: &[u8]);
    /// `TheDisplay->getWidth()`, `getHeight()`.
    fn display_size(&self) -> (i32, i32);
    /// `TheGameLogic->setGamePaused(paused, pauseMusic)`.
    fn set_game_paused(&mut self, paused: bool, pause_music: bool);
    /// `TheWindowManager->winCreateLayout(file)`.
    fn win_create_layout(&mut self, file: &[u8]) -> Option<usize>;
    /// `layout->runInit()`: the layout's init (for `InGamePopupMessage.wnd`, [`in_game_popup_message_init`]).
    fn layout_run_init(&mut self, layout: usize);
    /// `layout->destroyWindows()` and `deleteInstance()`.
    fn layout_destroy(&mut self, layout: usize);
    /// `TheNameKeyGenerator->nameToKey(name)`.
    fn name_to_key(&mut self, name: &[u8]) -> i32;
    /// `TheWindowManager->winGetWindowFromId(parent, id)`.
    fn win_get_window_from_id(&mut self, parent: Option<WinId>, id: i32) -> Option<WinId>;
    /// `window->winGetFont()`.
    fn win_get_font(&mut self, w: WinId) -> Option<GameFont>;
    /// A `DisplayString` with `text`, `font` and `setWordWrap(wrap)`: its `getSize(&width, &height)`.
    fn string_size(&mut self, text: &[u16], font: Option<&GameFont>, wrap: i32) -> (i32, i32);
    /// `GadgetStaticTextSetText(w, text)`.
    fn static_text_set_text(&mut self, w: WinId, text: &[u16]);
    /// `window->winGetSize(&width, &height)`.
    fn win_get_size(&mut self, w: WinId) -> (i32, i32);
    /// `window->winSetPosition(x, y)`, `winSetSize(width, height)`.
    fn win_set_position(&mut self, w: WinId, x: i32, y: i32);
    fn win_set_size(&mut self, w: WinId, width: i32, height: i32);
    /// `window->winSetEnabledTextColors(color, borderColor)`.
    fn win_set_enabled_text_colors(&mut self, w: WinId, color: i32, border_color: i32);
    /// `TheWindowManager->winSetModal(w)`, `winSetFocus(w)`; `window->winHide(hide)`, `winBringToTop()`.
    fn win_set_modal(&mut self, w: WinId);
    fn win_set_focus(&mut self, w: WinId);
    fn win_hide(&mut self, w: WinId, hide: bool);
    fn win_bring_to_top(&mut self, w: WinId);
    /// `TheWindowManager->winSendSystemMsg(window, GBM_SELECTED, (WindowMsgData)control, controlId)`.
    fn send_selected(&mut self, window: WinId, control: Option<WinId>, control_id: i32);
    /// `TheMessageStream->appendMessage(GameMessage::MSG_CLEAR_INGAME_POPUP_MESSAGE)`.
    fn append_clear_popup_message(&mut self);
}

/// `InGamePopupMessage.wnd`, the layout `popupMessage` makes.
pub const IN_GAME_POPUP_MESSAGE_WND: &[u8] = b"InGamePopupMessage.wnd";

/// `popupMessage(identifier, x, y, width, textColor, pause, pauseMusic)` (InGameUI.cpp:5169). The form without a colour
/// (InGameUI.cpp:5161) gives `m_popupMessageColor` (`InGameUI.ini`'s `PopupMessageColor`).
#[allow(clippy::too_many_arguments)]
pub fn popup_message<H: PopupMessageHost>(h: &mut H, identifier: &[u8], x: i32, y: i32, width: i32, text_color: i32, pause: bool, pause_music: bool) {
    if h.popup().popup_message_data.is_some() {
        clear_popup_message_data(h);
    }
    h.update_diplomacy_briefing_text(identifier);
    let message = h.fetch(identifier);
    // "x and why are passed in as a percentage of the screen, convert to screen coords"
    let x = x.clamp(0, if plant(9441) { 99 } else { 100 });
    let y = y.clamp(0, 100);
    let (dw, dh) = h.display_size();
    // `TheDisplay->getWidth() * (INT_TO_REAL(x) / 100)`: an `UnsignedInt` times a `Real`, stored in an `Int`
    let px = (dw as u32 as f32 * (x as f32 / 100.0f32)) as i32;
    let py = (dh as u32 as f32 * (y as f32 / 100.0f32)) as i32;
    // "cap the lower limit of the width"
    let width = if width < 50 && !plant(9442) { 50 } else { width };
    h.popup().popup_message_data = Some(PopupMessageData { message, x: px, y: py, width, text_color, pause, pause_music, layout: None });
    if pause {
        h.set_game_paused(true, pause_music);
    }
    create_popup_message_layout(h);
}

/// `popupMessage`'s last two statements (InGameUI.cpp:5204): `m_popupMessageData->layout =
/// TheWindowManager->winCreateLayout("InGamePopupMessage.wnd")`, then `layout->runInit()`. (#155: shared with
/// [`show_held_popup_message`].)
fn create_popup_message_layout<H: PopupMessageHost>(h: &mut H) {
    let layout = h.win_create_layout(IN_GAME_POPUP_MESSAGE_WND);
    if let Some(d) = h.popup().popup_message_data.as_mut() {
        d.layout = layout;
    }
    match layout {
        Some(l) => h.layout_run_init(l),
        None => h.popup().set_undefined("popupMessage: winCreateLayout gave NULL (layout->runInit)"),
    }
}

/// (#155) New: a popup message held without a window manager ([`WindowlessPopup`], its data made and its pause made
/// when it came), shown once the client has one (the shell is read): `m_popupMessageData` is that data and
/// `popupMessage`'s last two statements run, as if the layout had been made when the message came. Its pause and its
/// history label were made then, so neither is made again. A message already shown is cleared first, as `popupMessage`
/// clears one.
pub fn show_held_popup_message<H: PopupMessageHost>(h: &mut H, data: PopupMessageData) {
    if h.popup().popup_message_data.is_some() {
        clear_popup_message_data(h);
    }
    h.popup().popup_message_data = Some(PopupMessageData { layout: None, ..data });
    if plant(9486) {
        // (planted: the layout made, its init not run)
        let layout = h.win_create_layout(IN_GAME_POPUP_MESSAGE_WND);
        if let Some(d) = h.popup().popup_message_data.as_mut() {
            d.layout = layout;
        }
        return;
    }
    create_popup_message_layout(h);
}

/// (#155) A call the popup's code made on a client with no window manager, for the client to make
/// ([`WindowlessPopup::calls`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowlessCall {
    /// `TheGameLogic->setGamePaused(paused, pauseMusic)`.
    SetGamePaused { paused: bool, pause_music: bool },
    /// `UpdateDiplomacyBriefingText(label, FALSE)`.
    Briefing(Vec<u8>),
}

/// (#155) New: `TheInGameUI`'s popup message on a client that has no window manager: before the shell is read (the
/// owner's files still coming) and when there is none (no `MainMenu.wnd`). Upstream always has its window manager; what
/// it does without the window is read from `popupMessage` (InGameUI.cpp:5169): the message is held
/// (`m_popupMessageData`), its label goes to the briefing history and a pausing message pauses the game **before**
/// `winCreateLayout`, so all of that is done here as upstream does it, by the translated [`popup_message`]. Only the
/// window is not made: upstream would dereference the `NULL` layout `winCreateLayout` gives without the file
/// (`layout->runInit()`, a crash); here there is no window manager to ask, and nothing is recorded as undefined. A clear
/// (`MSG_CLEAR_INGAME_POPUP_MESSAGE`, `InGameUI::reset`) or the next message unpauses a pausing one through the
/// translated [`clear_popup_message_data`]. At most one message is held (upstream's one `m_popupMessageData`): the
/// client's popups no longer wait in a queue for a shell (bounded, #154). When a shell comes, the held message is shown
/// in it ([`WindowlessPopup::take_held`], [`show_held_popup_message`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WindowlessPopup {
    /// `m_popupMessageData` (and the window statics, which stay unset: no window is made).
    pub popup: InGamePopupMessage,
    /// `TheDisplay->getWidth()`, `getHeight()`.
    pub display: (i32, i32),
    /// The calls on the game and the briefing history made since the client last took them, in order.
    pub calls: Vec<WindowlessCall>,
}

/// [`WindowlessPopup`] as the popup's host, with `TheGameText->fetch`.
struct Windowless<'a, F: FnMut(&[u8]) -> Vec<u16>> {
    w: &'a mut WindowlessPopup,
    fetch: F,
}

impl<F: FnMut(&[u8]) -> Vec<u16>> PopupMessageHost for Windowless<'_, F> {
    fn popup(&mut self) -> &mut InGamePopupMessage {
        &mut self.w.popup
    }
    fn fetch(&mut self, label: &[u8]) -> Vec<u16> {
        (self.fetch)(label)
    }
    fn update_diplomacy_briefing_text(&mut self, label: &[u8]) {
        if !plant(9485) {
            self.w.calls.push(WindowlessCall::Briefing(label.to_vec()));
        }
    }
    fn display_size(&self) -> (i32, i32) {
        if plant(9487) {
            return (self.w.display.1, self.w.display.0);
        }
        self.w.display
    }
    fn set_game_paused(&mut self, paused: bool, pause_music: bool) {
        if !(paused && plant(9484)) {
            self.w.calls.push(WindowlessCall::SetGamePaused { paused, pause_music });
        }
    }
    /// No window manager: no layout.
    fn win_create_layout(&mut self, _file: &[u8]) -> Option<usize> {
        None
    }
    // (no layout is ever made, so nothing below is reached)
    fn layout_run_init(&mut self, _layout: usize) {}
    fn layout_destroy(&mut self, _layout: usize) {}
    fn name_to_key(&mut self, _name: &[u8]) -> i32 {
        0
    }
    fn win_get_window_from_id(&mut self, _parent: Option<WinId>, _id: i32) -> Option<WinId> {
        None
    }
    fn win_get_font(&mut self, _w: WinId) -> Option<GameFont> {
        None
    }
    fn string_size(&mut self, _text: &[u16], _font: Option<&GameFont>, _wrap: i32) -> (i32, i32) {
        (0, 0)
    }
    fn static_text_set_text(&mut self, _w: WinId, _text: &[u16]) {}
    fn win_get_size(&mut self, _w: WinId) -> (i32, i32) {
        (0, 0)
    }
    fn win_set_position(&mut self, _w: WinId, _x: i32, _y: i32) {}
    fn win_set_size(&mut self, _w: WinId, _width: i32, _height: i32) {}
    fn win_set_enabled_text_colors(&mut self, _w: WinId, _color: i32, _border_color: i32) {}
    fn win_set_modal(&mut self, _w: WinId) {}
    fn win_set_focus(&mut self, _w: WinId) {}
    fn win_hide(&mut self, _w: WinId, _hide: bool) {}
    fn win_bring_to_top(&mut self, _w: WinId) {}
    fn send_selected(&mut self, _window: WinId, _control: Option<WinId>, _control_id: i32) {}
    fn append_clear_popup_message(&mut self) {}
}

impl WindowlessPopup {
    /// `popupMessage(identifier, x, y, width, textColor, pause, pauseMusic)` without a window manager ([`popup_message`]
    /// on this host); `fetch` is `TheGameText->fetch`.
    #[allow(clippy::too_many_arguments)]
    pub fn popup_message(&mut self, fetch: impl FnMut(&[u8]) -> Vec<u16>, identifier: &[u8], x: i32, y: i32, width: i32, text_color: i32, pause: bool, pause_music: bool) {
        popup_message(&mut Windowless { w: self, fetch }, identifier, x, y, width, text_color, pause, pause_music);
        // (no window manager: the missing layout is this client's, not upstream's undefined behaviour)
        self.popup.undefined = None;
    }

    /// `clearPopupMessageData()` ([`clear_popup_message_data`] on this host).
    pub fn clear_popup_message_data(&mut self) {
        clear_popup_message_data(&mut Windowless { w: self, fetch: |_: &[u8]| Vec::new() });
    }

    /// The held message, for a window manager to show ([`show_held_popup_message`]); none is held after. Its pause stays
    /// made: the shown message's clear unpauses.
    pub fn take_held(&mut self) -> Option<PopupMessageData> {
        let d = self.popup.popup_message_data.take();
        if plant(9488) && d.as_ref().is_some_and(|d| d.pause) {
            self.calls.push(WindowlessCall::SetGamePaused { paused: false, pause_music: d.as_ref().is_some_and(|d| d.pause_music) });
        }
        d
    }
}

/// `clearPopupMessageData()` (InGameUI.cpp:5211).
pub fn clear_popup_message_data<H: PopupMessageHost>(h: &mut H) {
    let Some(d) = h.popup().popup_message_data.clone() else { return };
    if let Some(l) = d.layout {
        h.layout_destroy(l);
        if let Some(d) = h.popup().popup_message_data.as_mut() {
            d.layout = None;
        }
    }
    if d.pause && !plant(9443) {
        h.set_game_paused(false, d.pause_music);
    }
    h.popup().popup_message_data = None;
}

/// `InGamePopupMessageInit(layout, userData)` (InGamePopupMessage.cpp:82).
pub fn in_game_popup_message_init<H: PopupMessageHost>(h: &mut H) {
    let parent_id = h.name_to_key(b"InGamePopupMessage.wnd:InGamePopupMessageParent");
    let parent = h.win_get_window_from_id(None, parent_id);
    let static_text_message_id = h.name_to_key(b"InGamePopupMessage.wnd:StaticTextMessage");
    let static_text_message = h.win_get_window_from_id(parent, static_text_message_id);
    let button_ok_id = h.name_to_key(b"InGamePopupMessage.wnd:ButtonOk");
    let button_ok = h.win_get_window_from_id(parent, button_ok_id);
    {
        let p = h.popup();
        p.parent_id = parent_id;
        p.parent = parent;
        p.static_text_message_id = static_text_message_id;
        p.static_text_message = static_text_message;
        p.button_ok_id = button_ok_id;
        p.button_ok = button_ok;
    }
    let Some(pm_data) = h.popup().popup_message_data.clone() else {
        // ("We're in InGamePopupMessage without a pointer to pMData")
        return;
    };
    let (Some(parent), Some(static_text_message), Some(button_ok)) = (parent, static_text_message, button_ok) else {
        h.popup().set_undefined("InGamePopupMessageInit: a window of InGamePopupMessage.wnd is missing");
        return;
    };
    let font = h.win_get_font(static_text_message);
    let wrap = pm_data.width - if plant(9444) { 4 } else { 14 };
    let (_width, height) = h.string_size(&pm_data.message, font.as_ref(), wrap);
    h.static_text_set_text(static_text_message, &pm_data.message);
    // "set the positions/sizes"
    let (width_ok, height_ok) = h.win_get_size(button_ok);
    h.win_set_position(parent, pm_data.x, pm_data.y);
    h.win_set_size(parent, pm_data.width, height + 7 + 2 + 2 + height_ok + 2);
    h.win_set_position(static_text_message, 2, 2);
    h.win_set_size(static_text_message, pm_data.width - 4, height + 7);
    h.win_set_position(button_ok, pm_data.width - width_ok - 2, height + 7 + 2 + 2);
    h.win_set_enabled_text_colors(static_text_message, pm_data.text_color, 0);
    h.popup().pause = pm_data.pause;
    if pm_data.pause && !plant(9445) {
        h.win_set_modal(parent);
    }
    h.win_set_focus(parent);
    h.win_hide(parent, false);
    h.win_bring_to_top(parent);
}

/// `KEY_ESC` (`DIK_ESCAPE`), `KEY_ENTER` (`DIK_RETURN`), `KEY_STATE_UP`.
const KEY_ESC: u8 = 0x01;
const KEY_ENTER: u8 = 0x1C;
const KEY_STATE_UP: u32 = 0x0001;

/// `InGamePopupMessageInput(window, msg, mData1, mData2)` (InGamePopupMessage.cpp:147): `key` is `mData1`'s byte,
/// `state` `mData2`'s.
pub fn in_game_popup_message_input<H: PopupMessageHost>(h: &mut H, window: WinId, msg: u32, key: u8, state: u8) -> MsgHandled {
    use crate::client::window_manager::gwm;
    if msg == gwm::CHAR && (key == KEY_ENTER || (key == KEY_ESC && !plant(9446))) {
        // "send a simulated selected event to the parent window of the back/exit button"
        if u32::from(state) & KEY_STATE_UP != 0 {
            let (button_ok, button_ok_id) = (h.popup().button_ok, h.popup().button_ok_id);
            h.send_selected(window, button_ok, button_ok_id);
        }
        // "don't let key fall through anywhere else"
        return MsgHandled::Handled;
    }
    MsgHandled::Ignored
}

/// `InGamePopupMessageSystem(window, msg, mData1, mData2)` (InGamePopupMessage.cpp:192) for `GWM_CREATE`,
/// `GWM_DESTROY`, `GWM_INPUT_FOCUS` (`input_focus`: `mData1 == TRUE` and the `Bool` `mData2` points to; the window
/// wants the focus) and `GBM_SELECTED` of the control whose window id is `control_id` (`None`: `mData1` is `NULL`,
/// which upstream dereferences).
pub fn in_game_popup_message_system<H: PopupMessageHost>(h: &mut H, msg: u32, control_id: Option<i32>, input_focus: Option<(bool, &mut bool)>) -> MsgHandled {
    use crate::client::window_manager::{ggm, gwm};
    match msg {
        gwm::CREATE | gwm::DESTROY => MsgHandled::Handled,
        gwm::INPUT_FOCUS => {
            // "if we're givin the opportunity to take the keyboard focus we must say we want it"
            if let Some((true, f)) = input_focus {
                *f = !plant(9447);
            }
            MsgHandled::Handled
        }
        ggm::GBM_SELECTED => {
            let Some(control_id) = control_id else {
                h.popup().set_undefined("InGamePopupMessageSystem: GBM_SELECTED of a NULL control");
                return MsgHandled::Handled;
            };
            if control_id == h.popup().button_ok_id {
                if !h.popup().pause || plant(9448) {
                    h.append_clear_popup_message();
                } else {
                    clear_popup_message_data(h);
                }
            }
            MsgHandled::Handled
        }
        _ => MsgHandled::Ignored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A host recording the calls; the layout's three windows are 0, 1 (in 0) and 2 (in 0); the button is 40x20; text
    /// is 8 a character on lines `wrap` wide, 10 a line.
    #[derive(Default)]
    struct T {
        p: InGamePopupMessage,
        log: Vec<String>,
        paused: bool,
        layouts: usize,
    }
    impl PopupMessageHost for T {
        fn popup(&mut self) -> &mut InGamePopupMessage {
            &mut self.p
        }
        fn fetch(&mut self, label: &[u8]) -> Vec<u16> {
            label.iter().map(|&b| u16::from(b)).collect()
        }
        fn update_diplomacy_briefing_text(&mut self, label: &[u8]) {
            self.log.push(format!("brief {}", String::from_utf8_lossy(label)));
        }
        fn display_size(&self) -> (i32, i32) {
            (800, 600)
        }
        fn set_game_paused(&mut self, paused: bool, _pause_music: bool) {
            self.paused = paused;
        }
        fn win_create_layout(&mut self, _file: &[u8]) -> Option<usize> {
            self.layouts += 1;
            Some(self.layouts)
        }
        fn layout_run_init(&mut self, _layout: usize) {
            in_game_popup_message_init(self);
        }
        fn layout_destroy(&mut self, l: usize) {
            self.log.push(format!("destroy {l}"));
        }
        fn name_to_key(&mut self, name: &[u8]) -> i32 {
            match name {
                b"InGamePopupMessage.wnd:InGamePopupMessageParent" => 10,
                b"InGamePopupMessage.wnd:StaticTextMessage" => 11,
                _ => 12,
            }
        }
        fn win_get_window_from_id(&mut self, _parent: Option<WinId>, id: i32) -> Option<WinId> {
            Some((id - 10) as usize)
        }
        fn win_get_font(&mut self, _w: WinId) -> Option<GameFont> {
            None
        }
        fn string_size(&mut self, text: &[u16], _font: Option<&GameFont>, wrap: i32) -> (i32, i32) {
            let w = 8 * text.len() as i32;
            (w.min(wrap), 10 * ((w + wrap - 1) / wrap.max(1)).max(1))
        }
        fn static_text_set_text(&mut self, _w: WinId, _text: &[u16]) {}
        fn win_get_size(&mut self, _w: WinId) -> (i32, i32) {
            (40, 20)
        }
        fn win_set_position(&mut self, w: WinId, x: i32, y: i32) {
            self.log.push(format!("pos {w} {x} {y}"));
        }
        fn win_set_size(&mut self, w: WinId, width: i32, height: i32) {
            self.log.push(format!("size {w} {width} {height}"));
        }
        fn win_set_enabled_text_colors(&mut self, _w: WinId, _color: i32, _border_color: i32) {}
        fn win_set_modal(&mut self, w: WinId) {
            self.log.push(format!("modal {w}"));
        }
        fn win_set_focus(&mut self, _w: WinId) {}
        fn win_hide(&mut self, _w: WinId, _hide: bool) {}
        fn win_bring_to_top(&mut self, _w: WinId) {}
        fn send_selected(&mut self, _window: WinId, _control: Option<WinId>, control_id: i32) {
            let _ = in_game_popup_message_system(self, crate::client::window_manager::ggm::GBM_SELECTED, Some(control_id), None);
        }
        fn append_clear_popup_message(&mut self) {
            self.log.push("MSG_CLEAR_INGAME_POPUP_MESSAGE".into());
        }
    }

    /// #151, predicted before the run: a 30-character message 200 wide at (50 %, 150 %) is placed at (400, 600): y
    /// clamped to 100 %; it wraps at 186 into 2 lines (240 wide), 20 high: the box 200 x 53, the text 196 x 27, OK at (158,
    /// 31). A pausing one pauses the game and is modal; Enter released is OK, which clears it at once and unpauses. A
    /// non-pausing one's OK (Escape released) sends the clear message instead; Escape pressed (not released) only eats
    /// the key. A width of 10 is taken as 50.
    #[test]
    fn the_popup_lays_out_and_closes_as_upstream() {
        use crate::client::window_manager::gwm;
        let mut t = T::default();
        popup_message(&mut t, b"012345678901234567890123456789", 50, 150, 200, -1, true, false);
        assert!(t.paused);
        assert_eq!(t.p.popup_message_data.as_ref().map(|d| (d.x, d.y, d.width)), Some((400, 600, 200)));
        assert!(t.log.contains(&"size 0 200 53".to_string()) && t.log.contains(&"size 1 196 27".to_string()) && t.log.contains(&"pos 2 158 31".to_string()), "{:?}", t.log);
        assert!(t.log.contains(&"modal 0".to_string()));
        assert_eq!(in_game_popup_message_input(&mut t, 0, gwm::CHAR, KEY_ENTER, 1), MsgHandled::Handled);
        assert!(t.p.popup_message_data.is_none() && !t.paused);
        t.log.clear();
        popup_message(&mut t, b"Hi", -5, 10, 10, -1, false, false);
        assert_eq!(t.p.popup_message_data.as_ref().map(|d| (d.x, d.y, d.width)), Some((0, 60, 50)));
        assert_eq!(in_game_popup_message_input(&mut t, 0, gwm::CHAR, KEY_ESC, 0), MsgHandled::Handled);
        assert!(!t.log.iter().any(|l| l.starts_with("MSG_")));
        assert_eq!(in_game_popup_message_input(&mut t, 0, gwm::CHAR, KEY_ESC, 1), MsgHandled::Handled);
        assert!(t.log.iter().any(|l| l.starts_with("MSG_")) && t.p.popup_message_data.is_some());
        clear_popup_message_data(&mut t);
        assert!(t.p.popup_message_data.is_none() && t.log.contains(&"destroy 2".to_string()));
        assert_eq!(in_game_popup_message_input(&mut t, 0, gwm::CHAR, b'a', 1), MsgHandled::Ignored);
    }
}
