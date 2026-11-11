// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManager.cpp (processDestroyList,
//     PassSelectedButtonsToParentSystem, PassMessagesToParentSystem, update, linkWindow, insertWindowAheadOf,
//     unlinkWindow, unlinkChildWindow, isEnabled, isHidden, addWindowToParent, addWindowToParentAtEnd, windowHiding,
//     hideWindowsInRange, enableWindowsInRange, winCapture, winRelease, winGetCapture, winGetWindowFromId,
//     winSendSystemMsg, winSendInputMsg, winGetFocus, winSetFocus, winProcessKey, winProcessMouseEvent, drawWindow,
//     winRepaint, winCreate, winDestroy, winDestroyAll, winSetModal, winUnsetModal, winGetGrabWindow,
//     winSetGrabWindow, winSetLoneWindow, gogoGadgetPushButton, gogoGadgetCheckbox, gogoGadgetRadioButton,
//     gogoGadgetProgressBar, gogoGadgetStaticText, getWindowUnderCursor, winNextTab, winPrevTab, registerTabList,
//     clearTabList)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindow.cpp (winBringToTop, winActivate, winSetPosition,
//     winGetScreenPosition, winPointInWindow, winSetSize, winEnable, winHide, winSetStatus, winClearStatus,
//     winSetHiliteState, winSetParent, winIsChild, winSetOwner, winPointInChild, winPointInAnyChild,
//     GameWinDefaultInput, GameWinBlockInput, GameWinDefaultSystem, the empty winNextTab / winPrevTab)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManagerScript.cpp (createWindow's and createGadget's
//     calls into the manager: the callbacks, the owner, GWM_SCRIPT_CREATE)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/WindowLayout.cpp (load, addWindow, removeWindow, hide,
//     destroyWindows, bringForward, runInit / runUpdate / runShutdown)
//   GeneralsMD/Code/GameEngine/Source/Common/System/FunctionLexicon.cpp and
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/Common/System/W3DFunctionLexicon.cpp (the tables' entries
//     this translation has: name to callback)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Input/Mouse.cpp (setCursorTooltip, resetTooltipDelay, the tooltip
//     timer of createStreamMessages, drawTooltip's condition)
//   GeneralsMD/Code/GameEngine/Include/GameClient/{GameWindow.h, Gadget.h, KeyDefs.h} (the messages, the keys)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   GameWindowManager.cpp, GameWindow.cpp: Colin Day, 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The window manager (GitHub #34): `TheWindowManager`, which owns every window of the game's UI, sends them their
//! messages, routes the mouse and the keyboard to them, and draws them. It runs the windows that
//! [`crate::client::window_script`] reads from the WND layouts.
//!
//! # As upstream reads it
//!
//! A window is created by `winCreate(parent, status, x, y, w, h, system, instData)`: a child goes to the **head** of
//! its parent's child list (`addWindowToParent`), a top-level window to the head of the window list (`linkWindow`),
//! but behind the last modal window if there is one. The system callback gets `GWM_CREATE` before the instance data
//! is copied in. A gadget (`gogoGadgetPushButton`, ...) then gets its input and draw callbacks (the draw one chosen by
//! `WIN_STATUS_IMAGE`), its owner (the parent; messages such as `GBM_SELECTED` go to the owner's system callback) and
//! its user data. The script reader then replaces each callback the file named and the lexicon has, sets the text,
//! and sends the parent `GWM_SCRIPT_CREATE` as an **input** message.
//!
//! Every message goes through `winSendSystemMsg` or `winSendInputMsg`, which ignore a destroyed window (except for
//! `GWM_DESTROY`). The mouse (`winProcessMouseEvent`): a captor gets everything, the innermost child under the pointer
//! first and then up to the captor; else a **grab window** (the window that took the last left press) gets the drag and
//! the release (the release only when the pointer is still over it, or the window is draggable); else the window under
//! the pointer is found in the modal window, or among the top-level windows in three passes (`ABOVE`, then neither,
//! then `BELOW`; hidden ones skipped, a disabled one passed over), down its enabled, shown children. A `NO_INPUT`
//! window gives way to its parent if that is a combo box, else to nothing. The message goes to the window and up its
//! parents until one handles it; the one that handles a left press becomes the grab window. A **lone window** (an
//! open combo box's list) is closed by any click that some other window took. Then the window that the pointer left
//! gets `GWM_MOUSE_LEAVING` and the new one `GWM_MOUSE_ENTERING`. The tooltip: the first window under the pointer with
//! a tooltip callback or text, else the window itself; its callback runs, or its text is set into the mouse's tooltip
//! with its delay.
//!
//! The keyboard (`winProcessKey`) goes to the focus window and up its parents until one handles it. `winSetFocus`
//! tells the old focus window it loses the focus, then asks the new one and its parents in turn whether they want it
//! (`GWM_INPUT_FOCUS`, with a `Bool*` answer); if none does, nothing has the focus.
//!
//! `winDestroy` marks the window and its children destroyed, takes them off their lists and puts them on the destroy
//! list; `update` (`processDestroyList`) sends each `GWM_DESTROY` and frees it. `winRepaint` draws the window list from
//! its tail (the oldest) in three passes, `BELOW`, neither, `ABOVE`; `drawWindow` draws a shown window (unless
//! see-through), its children from the tail of its list (so the newest is on top), then its border.
//!
//! The mouse's tooltip shows when the mouse has been still for the window's delay (or the mouse's `TooltipDelayTime`,
//! 50 ms by default), measured in `createStreamMessages` before the frame's own moves.
//!
//! # Upstream quirks that are kept
//!
//! - `winSetFocus` keeps the **asked** window as the focus even when it was a parent that said it wanted it.
//! - `GameWindow::winNextTab` / `winPrevTab` are empty (their bodies are commented out): the radio button and static
//!   text use them, so their arrow keys and tab do nothing; the push button and check box use the manager's
//!   `winNextTab`, which walks the registered tab list (and does nothing while a modal window is up).
//! - `winProcessMouseEvent` clears the mouse's tooltip at every mouse event; a window found in a top-level window that
//!   is disabled still offers its tooltip; the tooltip search uses `winPointInAnyChild`, which ignores the enabled bit.
//! - A left release over a disabled child of the grab window plays `GUIClickDisabled`; so does nothing else.
//! - `GadgetRadioSetSelection` passes a **pointer** as `mData1` and the radio's system compares `(Bool)mData1 == TRUE`:
//!   the selected message is never sent (see [`crate::client::gadget`]).
//! - `GWM_LEFT_DRAG` moves a draggable grab window clipped to its parent's size, then to the display.
//! - A window keeps its draw function's choice of `WIN_STATUS_IMAGE` from its creation: setting the bit later does
//!   not switch a gadget to its image drawing.
//!
//! # Where this differs from upstream
//!
//! - The reader ([`crate::client::window_script`]) builds every window of a file first; this manager then **replays**
//!   upstream's calls for each window in creation order ([`GameWindowManager::win_create_from_script`]): the link,
//!   `GWM_CREATE`, the gadget's callbacks, owner and user data, the file's callbacks, the text, `GWM_SCRIPT_CREATE`.
//!   The messages are the same and in the same order; a callback that looked at windows read later in the same file
//!   would see them already there (no translated callback does). The gadget's label is set once, to its final text
//!   (upstream sets the raw label first, then the fetched text).
//! - The callbacks this translation does not have (the game's own menus, `ControlBarSystem`, the device draws of the
//!   HUD) are named, not called: [`GuiHost`] gets them by name. ~~The gadgets not translated yet (list box, combo box,
//!   sliders, text entry, tab control) get no behaviour (their messages are ignored) and are drawn with
//!   `W3DGameWinDefaultDraw`; their inner windows are not created (see #34's remaining list).~~ **Corrected 2026-11-10
//!   (#70):** the list box and the sliders are translated ([`crate::client::gadget_list_box`],
//!   [`crate::client::gadget_slider`]), with their inner windows (the scroll bar's buttons and slider, the thumbs),
//!   created after the file's children of the gadget in the child list as upstream's are; ~~the combo box, text entry
//!   and tab control still get no behaviour and are drawn with `W3DGameWinDefaultDraw`, and the progress bar's draw is
//!   that too (see #70's remaining list).~~ **Corrected 2026-11-10 (#72):** the text entry, combo box and tab control
//!   are translated too ([`crate::client::gadget_text_entry`], [`crate::client::gadget_combo_box`],
//!   [`crate::client::gadget_tab_control`]), with their inner windows (the combo box's drop-down button, edit box and
//!   list; the tab control's panes, which a file with children replaces: `parseChildWindows`), and every gadget has
//!   its W3D draw; `assignDefaultGadgetLook`'s default visual is translated ([`crate::client::gadget_look`]) and given
//!   where upstream asks for it (the gadgets' inner buttons, thumbs and sliders; [`GameWindowManager::gogo_gadget_look`]
//!   with `default_visual`).
//! - Where upstream's behaviour is undefined (a `NULL` or out-of-range access in a gadget), the manager records it in
//!   [`GameWindowManager::undefined`] and the message stops. **Added 2026-11-10 (#72):** so does a mouse event while
//!   the top of the modal stack is a freed window (`winDestroy` unsets only the top: a window destroyed below it stays
//!   in the stack, and once the windows above it are unset upstream searches freed memory). `getWindowUnderCursor`
//!   reads it as it is (it cannot record).
//! - Window data that upstream passes as pointers: a window is `id + 1` (0 for `NULL`, [`win_data`]); a string
//!   (`GGM_SET_LABEL`, `GGM_GET_LABEL`) is in [`GameWindowManager::msg_text`]; the `Bool*` of `GWM_INPUT_FOCUS` is
//!   [`GameWindowManager::focus_reply`].
//! - Audio (`GUIClick`), the selection translator's state that `GameWinBlockInput` clears, text measuring and image
//!   sizes are asked of the [`GuiHost`].
//! - **Added 2026-11-10 (#74):** `TheIMEManager` (a global upstream) is the manager's [`GameWindowManager::ime`]
//!   ([`crate::client::ime_manager`]), so the text entry and the shell reach it through the manager.
//! - `WIN_MAX_WINDOWS` is not enforced (upstream's pool allocator has no such limit either).
//! - Windows freed by `processDestroyList` stay in the vector, marked freed, so ids stay stable.
//!
//! # Representation
//!
//! [`GameWindowManager`] holds the windows ([`WindowTree`], the reader's), each one's callbacks, owner, layout and
//! user data ([`WinExt`]), the window list (head first: the newest top-level window, drawn last and hit first), the
//! destroy list, the modal stack (top first), and the mouse's tooltip ([`MouseTooltip`]). A window's children are in
//! its `children` vector, head first.

use crate::client::window_script::{gws, win_status, GadgetData, GameFont, GameWindow, RadioButtonData, ScriptResult, TextData, WinId, WinInstanceData, WindowLayoutInfo, WindowScript, WindowTree, WndError, WndHost};

/// `WindowMsgData` (`UnsignedInt`).
pub type WindowMsgData = u32;

/// `WindowMsgHandledType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MsgHandled {
    Ignored,
    Handled,
}

/// `WinInputReturnCode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WinInputReturnCode {
    NotUsed,
    Used,
}

/// `GameWindowMessage`.
pub mod gwm {
    pub const NONE: u32 = 0;
    pub const CREATE: u32 = 1;
    pub const DESTROY: u32 = 2;
    pub const ACTIVATE: u32 = 3;
    pub const ENABLE: u32 = 4;
    pub const LEFT_DOWN: u32 = 5;
    pub const LEFT_UP: u32 = 6;
    pub const LEFT_DOUBLE_CLICK: u32 = 7;
    pub const LEFT_DRAG: u32 = 8;
    pub const MIDDLE_DOWN: u32 = 9;
    pub const MIDDLE_UP: u32 = 10;
    pub const MIDDLE_DOUBLE_CLICK: u32 = 11;
    pub const MIDDLE_DRAG: u32 = 12;
    pub const RIGHT_DOWN: u32 = 13;
    pub const RIGHT_UP: u32 = 14;
    pub const RIGHT_DOUBLE_CLICK: u32 = 15;
    pub const RIGHT_DRAG: u32 = 16;
    pub const MOUSE_ENTERING: u32 = 17;
    pub const MOUSE_LEAVING: u32 = 18;
    pub const WHEEL_UP: u32 = 19;
    pub const WHEEL_DOWN: u32 = 20;
    pub const CHAR: u32 = 21;
    pub const SCRIPT_CREATE: u32 = 22;
    pub const INPUT_FOCUS: u32 = 23;
    pub const MOUSE_POS: u32 = 24;
    pub const IME_CHAR: u32 = 25;
    pub const IME_STRING: u32 = 26;
    /// `GWM_USER`.
    pub const USER: u32 = 32768;
}

/// `GadgetGameMessage` (Gadget.h), in its order from `GGM_LEFT_DRAG = 16384`.
pub mod ggm {
    pub const LEFT_DRAG: u32 = 16384;
    pub const SET_LABEL: u32 = 16385;
    pub const GET_LABEL: u32 = 16386;
    pub const FOCUS_CHANGE: u32 = 16387;
    pub const RESIZED: u32 = 16388;
    pub const CLOSE: u32 = 16389;
    pub const GBM_MOUSE_ENTERING: u32 = 16390;
    pub const GBM_MOUSE_LEAVING: u32 = 16391;
    pub const GBM_SELECTED: u32 = 16392;
    pub const GBM_SELECTED_RIGHT: u32 = 16393;
    pub const GBM_SET_SELECTION: u32 = 16394;
    pub const GSM_SLIDER_TRACK: u32 = 16395;
    pub const GSM_SET_SLIDER: u32 = 16396;
    pub const GSM_SET_MIN_MAX: u32 = 16397;
    pub const GSM_SLIDER_DONE: u32 = 16398;
    pub const GLM_ADD_ENTRY: u32 = 16399;
    pub const GLM_SELECTED: u32 = 16402;
    pub const GCM_SELECTED: u32 = 16419;
    pub const GEM_EDIT_DONE: u32 = 16430;
    pub const GPM_SET_PROGRESS: u32 = 16432;
}

/// The keys the gadgets look at (`KeyDefs.h`: DirectInput's scan codes).
pub mod key {
    pub const NONE: u8 = 0x00;
    pub const ESC: u8 = 0x01;
    pub const BACKSPACE: u8 = 0x0E;
    pub const TAB: u8 = 0x0F;
    pub const ENTER: u8 = 0x1C;
    pub const SPACE: u8 = 0x39;
    pub const HOME: u8 = 0xC7;
    pub const UP: u8 = 0xC8;
    pub const LEFT: u8 = 0xCB;
    pub const RIGHT: u8 = 0xCD;
    pub const END: u8 = 0xCF;
    pub const DOWN: u8 = 0xD0;
    pub const DEL: u8 = 0xD3;
    /// `KEY_STATE_UP`.
    pub const STATE_UP: u32 = 0x0001;
    /// `KEY_STATE_DOWN`.
    pub const STATE_DOWN: u32 = 0x0002;
}

/// `WIN_STATE_HILITED`.
pub const WIN_STATE_HILITED: i32 = 0x0000_0002;
/// `WIN_STATE_SELECTED`.
pub const WIN_STATE_SELECTED: i32 = 0x0000_0004;
/// `WIN_STATUS_SHORTCUT_BUTTON` (not in the reader's names).
pub const WIN_STATUS_SHORTCUT_BUTTON: u32 = 0x0400_0000;

/// `WIN_ERR_OK`.
pub const WIN_ERR_OK: i32 = 0;
/// `WIN_ERR_GENERAL_FAILURE`.
pub const WIN_ERR_GENERAL_FAILURE: i32 = -1;
/// `WIN_ERR_INVALID_WINDOW`.
pub const WIN_ERR_INVALID_WINDOW: i32 = -2;
/// `WIN_ERR_INVALID_PARAMETER`.
pub const WIN_ERR_INVALID_PARAMETER: i32 = -3;
/// `WIN_ERR_MOUSE_CAPTURED`.
pub const WIN_ERR_MOUSE_CAPTURED: i32 = -4;

/// `sendMousePosMessages` (TRUE in the release).
pub const SEND_MOUSE_POS_MESSAGES: bool = true;

/// `SHORTTOLONG(x, y)`.
pub fn short_to_long(x: i32, y: i32) -> u32 {
    u32::from(x as u16) | (u32::from(y as u16) << 16)
}

/// A window as message data (`(WindowMsgData)window`): `id + 1`, 0 for `NULL`.
pub fn win_data(w: Option<WinId>) -> WindowMsgData {
    w.map_or(0, |w| w as u32 + 1)
}

/// The window a [`win_data`] names.
pub fn data_win(d: WindowMsgData) -> Option<WinId> {
    (d != 0).then(|| d as usize - 1)
}

/// A system callback (`GameWinSystemFunc`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemFunc {
    /// `GameWinDefaultSystem`.
    Default,
    PassSelectedButtonsToParent,
    PassMessagesToParent,
    PushButton,
    CheckBox,
    RadioButton,
    StaticText,
    ProgressBar,
    /// `GadgetListBoxSystem`.
    ListBox,
    /// `GadgetVerticalSliderSystem`.
    VerticalSlider,
    /// `GadgetHorizontalSliderSystem`.
    HorizontalSlider,
    /// `GadgetTextEntrySystem`.
    TextEntry,
    /// `GadgetComboBoxSystem`.
    ComboBox,
    /// `GadgetTabControlSystem`.
    TabControl,
    /// A callback of the game's own (`ControlBarSystem`, `MainMenuSystem`, ...), for the [`GuiHost`].
    Host(Vec<u8>),
}

/// An input callback (`GameWinInputFunc`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputFunc {
    /// `GameWinDefaultInput`.
    Default,
    /// `GameWinBlockInput`.
    Block,
    PushButton,
    CheckBox,
    RadioButton,
    StaticText,
    /// `GadgetListBoxInput`.
    ListBox,
    /// `GadgetListBoxMultiInput`.
    ListBoxMulti,
    VerticalSlider,
    HorizontalSlider,
    /// `GadgetTextEntryInput`.
    TextEntry,
    /// `GadgetComboBoxInput`.
    ComboBox,
    /// `GadgetTabControlInput`.
    TabControl,
    Host(Vec<u8>),
}

/// A draw callback (`GameWinDrawFunc`), the W3D device's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrawFunc {
    /// `GameWinDefaultDraw`: draws nothing.
    GameWinDefault,
    /// `W3DGameWinDefaultDraw` (the W3D manager's default).
    W3DDefault,
    PushButton,
    PushButtonImage,
    CheckBox,
    CheckBoxImage,
    RadioButton,
    RadioButtonImage,
    StaticText,
    StaticTextImage,
    ListBox,
    ListBoxImage,
    VerticalSlider,
    VerticalSliderImage,
    HorizontalSlider,
    HorizontalSliderImage,
    /// `W3DNoDraw`.
    NoDraw,
    TabControl,
    TabControlImage,
    ComboBox,
    ComboBoxImage,
    ProgressBar,
    ProgressBarImage,
    TextEntry,
    TextEntryImage,
    Host(Vec<u8>),
}

/// The function lexicon's system table, as far as it is translated (`FunctionLexicon::gameWinSystemTable`): the
/// engine's own names; any other name the host resolved is the host's.
pub fn lexicon_system(name: &[u8]) -> SystemFunc {
    match name {
        b"PassSelectedButtonsToParentSystem" => SystemFunc::PassSelectedButtonsToParent,
        b"PassMessagesToParentSystem" => SystemFunc::PassMessagesToParent,
        b"GameWinDefaultSystem" => SystemFunc::Default,
        b"GadgetPushButtonSystem" => SystemFunc::PushButton,
        b"GadgetCheckBoxSystem" => SystemFunc::CheckBox,
        b"GadgetRadioButtonSystem" => SystemFunc::RadioButton,
        b"GadgetStaticTextSystem" => SystemFunc::StaticText,
        b"GadgetProgressBarSystem" => SystemFunc::ProgressBar,
        b"GadgetTabControlSystem" => SystemFunc::TabControl,
        b"GadgetListBoxSystem" => SystemFunc::ListBox,
        b"GadgetComboBoxSystem" => SystemFunc::ComboBox,
        b"GadgetHorizontalSliderSystem" => SystemFunc::HorizontalSlider,
        b"GadgetVerticalSliderSystem" => SystemFunc::VerticalSlider,
        b"GadgetTextEntrySystem" => SystemFunc::TextEntry,
        _ => SystemFunc::Host(name.to_vec()),
    }
}

/// The lexicon's input table (`gameWinInputTable`), as [`lexicon_system`].
pub fn lexicon_input(name: &[u8]) -> InputFunc {
    match name {
        b"GameWinDefaultInput" => InputFunc::Default,
        b"GameWinBlockInput" => InputFunc::Block,
        b"GadgetPushButtonInput" => InputFunc::PushButton,
        b"GadgetCheckBoxInput" => InputFunc::CheckBox,
        b"GadgetRadioButtonInput" => InputFunc::RadioButton,
        b"GadgetStaticTextInput" => InputFunc::StaticText,
        b"GadgetTabControlInput" => InputFunc::TabControl,
        b"GadgetListBoxInput" => InputFunc::ListBox,
        b"GadgetListBoxMultiInput" => InputFunc::ListBoxMulti,
        b"GadgetComboBoxInput" => InputFunc::ComboBox,
        b"GadgetHorizontalSliderInput" => InputFunc::HorizontalSlider,
        b"GadgetVerticalSliderInput" => InputFunc::VerticalSlider,
        b"GadgetTextEntryInput" => InputFunc::TextEntry,
        _ => InputFunc::Host(name.to_vec()),
    }
}

/// The lexicon's draw tables (`W3DFunctionLexicon`'s device table first, then `gameWinDrawTable`), as
/// [`lexicon_system`].
pub fn lexicon_draw(name: &[u8]) -> DrawFunc {
    match name {
        b"GameWinDefaultDraw" => DrawFunc::GameWinDefault,
        b"W3DGameWinDefaultDraw" => DrawFunc::W3DDefault,
        b"W3DGadgetPushButtonDraw" => DrawFunc::PushButton,
        b"W3DGadgetPushButtonImageDraw" => DrawFunc::PushButtonImage,
        b"W3DGadgetCheckBoxDraw" => DrawFunc::CheckBox,
        b"W3DGadgetCheckBoxImageDraw" => DrawFunc::CheckBoxImage,
        b"W3DGadgetRadioButtonDraw" => DrawFunc::RadioButton,
        b"W3DGadgetRadioButtonImageDraw" => DrawFunc::RadioButtonImage,
        b"W3DGadgetStaticTextDraw" => DrawFunc::StaticText,
        b"W3DGadgetStaticTextImageDraw" => DrawFunc::StaticTextImage,
        b"W3DNoDraw" => DrawFunc::NoDraw,
        b"W3DGadgetTabControlDraw" => DrawFunc::TabControl,
        b"W3DGadgetTabControlImageDraw" => DrawFunc::TabControlImage,
        b"W3DGadgetListBoxDraw" => DrawFunc::ListBox,
        b"W3DGadgetListBoxImageDraw" => DrawFunc::ListBoxImage,
        b"W3DGadgetComboBoxDraw" => DrawFunc::ComboBox,
        b"W3DGadgetComboBoxImageDraw" => DrawFunc::ComboBoxImage,
        b"W3DGadgetHorizontalSliderDraw" => DrawFunc::HorizontalSlider,
        b"W3DGadgetHorizontalSliderImageDraw" => DrawFunc::HorizontalSliderImage,
        b"W3DGadgetVerticalSliderDraw" => DrawFunc::VerticalSlider,
        b"W3DGadgetVerticalSliderImageDraw" => DrawFunc::VerticalSliderImage,
        b"W3DGadgetProgressBarDraw" => DrawFunc::ProgressBar,
        b"W3DGadgetProgressBarImageDraw" => DrawFunc::ProgressBarImage,
        b"W3DGadgetTextEntryDraw" => DrawFunc::TextEntry,
        b"W3DGadgetTextEntryImageDraw" => DrawFunc::TextEntryImage,
        _ => DrawFunc::Host(name.to_vec()),
    }
}

/// `NO_CLOCK`, `NORMAL_CLOCK`, `INVERSE_CLOCK`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClockKind {
    #[default]
    None,
    Normal,
    Inverse,
}

/// `PushButtonData` (`userData` is the caller's own and not modelled).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PushButtonData {
    pub draw_clock: ClockKind,
    pub percent_clock: i32,
    pub color_clock: i32,
    pub draw_border: bool,
    pub color_border: i32,
    /// The overlay image's name.
    pub overlay_image: Option<Vec<u8>>,
    pub alt_sound: Vec<u8>,
}

/// A window's user data (`winGetUserData`), by gadget.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum UserData {
    #[default]
    None,
    /// A push button's (`NULL` until a `GadgetButton*` setter makes one: `None` inside).
    PushButton(Option<PushButtonData>),
    Radio(RadioButtonData),
    /// A static text's `TextData`, with its display string's text.
    StaticText(TextData, Vec<u16>),
    /// A progress bar's position (`winSetUserData((void *)newPos)`).
    Progress(i32),
    /// A list box's `ListboxData`.
    Listbox(crate::client::gadget_list_box::ListboxRt),
    /// A slider's `SliderData`.
    Slider(crate::client::gadget_slider::SliderRt),
    /// A text entry's `EntryData`.
    Entry(crate::client::gadget_text_entry::EntryRt),
    /// A combo box's `ComboBoxData`.
    Combo(crate::client::gadget_combo_box::ComboRt),
    /// A tab control's `TabControlData`.
    Tab(crate::client::gadget_tab_control::TabRt),
    /// (#69) A message box's `WindowMessageBoxData` (`gogoMessageBox`).
    MessageBox(crate::client::message_box::MessageBoxData),
    /// (#89) A map preview's `MapMetaData *` (`winSetUserData((void *)TheMapCache->findMap(name))`): the map's key in
    /// the map cache (`None`: `NULL`).
    MapMetaData(Option<Vec<u8>>),
}

/// What the manager keeps for a window besides the reader's [`GameWindow`].
#[derive(Clone, Debug, PartialEq)]
pub struct WinExt {
    pub system: SystemFunc,
    pub input: InputFunc,
    pub draw: DrawFunc,
    /// `m_tooltip`: a host's tooltip callback, by name.
    pub tooltip: Option<Vec<u8>>,
    /// `m_instData.m_owner` (`None`: `NULL`).
    pub owner: Option<WinId>,
    /// `m_layout`.
    pub layout: Option<usize>,
    pub user: UserData,
    /// Freed by `processDestroyList`.
    pub freed: bool,
}

impl Default for WinExt {
    /// `GameWindow::GameWindow()` on the W3D manager: the default callbacks, no tooltip callback.
    fn default() -> Self {
        WinExt { system: SystemFunc::Default, input: InputFunc::Default, draw: DrawFunc::W3DDefault, tooltip: None, owner: None, layout: None, user: UserData::None, freed: false }
    }
}

/// `WindowLayout`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WindowLayout {
    pub filename: Vec<u8>,
    /// `m_windowList`, head first (`addWindow` puts at the head).
    pub windows: Vec<WinId>,
    pub hidden: bool,
    /// The init, update and shutdown callbacks the lexicon resolved, by name.
    pub init: Option<Vec<u8>>,
    pub update: Option<Vec<u8>>,
    pub shutdown: Option<Vec<u8>>,
    /// (#69) `deleteInstance`d (the shell's popped screens): the index stays, the layout is not used again.
    pub deleted: bool,
}

/// The mouse's tooltip: ~~here~~ **Corrected 2026-11-11 (#124):** translated in full with the mouse
/// ([`crate::client::mouse::MouseTooltip`]: `setCursorTooltip` with its width and colour, the timer, `drawTooltip`).
pub use crate::client::mouse::MouseTooltip;

/// One thing to draw (`TheDisplay`'s calls), in screen pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrawItem {
    /// `drawFillRect(x, y, w, h, color)`.
    FillRect { x: i32, y: i32, w: i32, h: i32, color: i32 },
    /// `drawOpenRect(x, y, w, h, width, color)`.
    OpenRect { x: i32, y: i32, w: i32, h: i32, color: i32 },
    /// `drawLine(x0, y0, x1, y1, width, color)`.
    Line { x0: i32, y0: i32, x1: i32, y1: i32, color: i32 },
    /// `drawImage(image, x0, y0, x1, y1, color, mode)`; `clip` the clip region set at the time.
    Image { image: Vec<u8>, x0: i32, y0: i32, x1: i32, y1: i32, color: u32, grayscale: bool, clip: Option<(i32, i32, i32, i32)> },
    /// `DisplayString::draw(x, y, color, dropColor)`.
    Text { text: Vec<u16>, x: i32, y: i32, color: i32, drop_color: i32, font: Option<GameFont>, word_wrap: Option<i32>, wrap_centered: bool, hotkey: bool, clip: Option<(i32, i32, i32, i32)> },
    /// `drawRectClock` / `drawRemainingRectClock`.
    RectClock { x: i32, y: i32, w: i32, h: i32, percent: i32, color: i32, inverse: bool },
}

/// What the game supplies to the manager: its own callbacks (by name), audio, text measuring, image sizes.
pub trait GuiHost {
    /// A system callback of the game's own.
    fn system(&mut self, _wm: &mut GameWindowManager, _func: &[u8], _win: WinId, _msg: u32, _d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
        MsgHandled::Ignored
    }
    /// An input callback of the game's own.
    fn input(&mut self, _wm: &mut GameWindowManager, _func: &[u8], _win: WinId, _msg: u32, _d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
        MsgHandled::Ignored
    }
    /// A tooltip callback (`m_tooltip(window, instData, mouse)`).
    fn tooltip(&mut self, _wm: &mut GameWindowManager, _func: &[u8], _win: WinId, _mouse: u32) {}
    /// A draw callback of the game's own.
    fn draw(&mut self, _wm: &GameWindowManager, _func: &[u8], _win: WinId, _out: &mut Vec<DrawItem>) {}
    /// A layout's init, update or shutdown callback (`runInit`, ...).
    fn layout(&mut self, _wm: &mut GameWindowManager, _func: &[u8], _layout: usize) {}
    /// `TheAudio->addAudioEvent(name)`.
    fn add_audio_event(&mut self, _name: &[u8]) {}
    /// `GameWinBlockInput`'s left release: the selection translator stops drag selecting, the view's mouse lock ends,
    /// the in-game UI stops selecting.
    fn block_input_left_up(&mut self) {}
    /// `DisplayString::getSize` of `text` in `font`, word wrapped at `wrap` pixels when given.
    fn text_size(&self, font: Option<&GameFont>, text: &[u16], wrap: Option<i32>) -> (i32, i32) {
        let h = font.map_or(12, |f| f.point_size.max(1));
        let w = text.len() as i32 * (h / 2).max(1);
        match wrap {
            Some(wr) if wr > 0 && w > wr => (wr, h * ((w + wr - 1) / wr)),
            _ => (w, h),
        }
    }
    /// `winFontHeight(font)` (`font->height`).
    fn font_height(&self, font: Option<&GameFont>) -> i32 {
        font.map_or(12, |f| f.point_size.max(1))
    }
    /// `TheKeyboard->getPrintableKey(key, state)` (0, 1, 2: plain, shifted, the second shift); 0 for none.
    fn printable_key(&self, _key: u8, _state: i32) -> u16 {
        0
    }
    /// `image->getImageWidth()`, `getImageHeight()`.
    fn image_size(&self, _image: &[u8]) -> Option<(i32, i32)> {
        None
    }
    /// `TheMappedImageCollection->findImageByName(name)` found it.
    fn has_image(&self, _image: &[u8]) -> bool {
        false
    }
    /// (#77) `TheControlBar->getArrowImage()` (the scheme's `GenArrow`), by name; `None` for `NULL`.
    fn control_bar_arrow_image(&self) -> Option<Vec<u8>> {
        None
    }
}

/// A host with none of the game's callbacks.
pub struct NoGuiHost;
impl GuiHost for NoGuiHost {}

/// `TheWindowManager`.
#[derive(Clone, Debug, Default)]
pub struct GameWindowManager {
    pub tree: WindowTree,
    pub ext: Vec<WinExt>,
    /// `m_windowList` .. `m_windowTail`, head first.
    pub window_list: Vec<WinId>,
    /// `m_destroyList`, head first.
    pub destroy_list: Vec<WinId>,
    pub curr_mouse_rgn: Option<WinId>,
    pub mouse_captor: Option<WinId>,
    pub keyboard_focus: Option<WinId>,
    /// `m_modalHead` and its `next`s, top first.
    pub modal: Vec<WinId>,
    pub grab_window: Option<WinId>,
    pub lone_window: Option<WinId>,
    /// `m_tabList`.
    pub tab_list: Vec<WinId>,
    pub layouts: Vec<WindowLayout>,
    /// `TheMouse`'s tooltip.
    pub mouse: MouseTooltip,
    /// `TheDisplay->getWidth()`, `getHeight()`.
    pub display: (i32, i32),
    /// The string a `GGM_SET_LABEL` / `GGM_GET_LABEL` points to.
    pub msg_text: Vec<u16>,
    /// The `Bool` a `GWM_INPUT_FOCUS`'s `mData2` points to.
    pub focus_reply: bool,
    /// `Display::setClipRegion` while drawing.
    pub(crate) clip: Option<(i32, i32, i32, i32)>,
    /// The first operation upstream leaves undefined that the translation met (a `NULL` or out-of-range access, a
    /// freed array used): the message that met it stops there, and a differential stops the session.
    pub undefined: Option<&'static str>,
    /// The data the list box's messages point to.
    pub list_msg: crate::client::gadget_list_box::ListMsg,
    /// `timeGetTime()`, milliseconds (the list box's double click), set by the host.
    pub time_ms: u32,
    /// The string the text entry's messages point to.
    pub entry_msg: crate::client::gadget_text_entry::EntryMsg,
    /// The data the combo box's messages point to.
    pub combo_msg: crate::client::gadget_combo_box::ComboMsg,
    /// `drawTextEntryText`'s `static Byte drawCnt` (the caret's blink), one for every text entry.
    pub(crate) entry_draw_cnt: u8,
    /// (#74) `TheIMEManager`: the text entry typed characters go to ([`crate::client::ime_manager`]).
    pub ime: crate::client::ime_manager::ImeManager,
    /// (#77) `TheTransitionHandler`, which the manager creates, updates, draws and resets
    /// ([`crate::client::window_transitions`]).
    pub transitions: crate::client::window_transitions::GameWindowTransitionsHandler,
}

/// The gadget kinds this translation creates, from the style bits (`createGadget`'s order).
fn gadget_style(style: u32) -> u32 {
    for b in [gws::PUSH_BUTTON, gws::RADIO_BUTTON, gws::CHECK_BOX, gws::TAB_CONTROL, gws::VERT_SLIDER, gws::HORZ_SLIDER, gws::SCROLL_LISTBOX, gws::COMBO_BOX, gws::PROGRESS_BAR, gws::STATIC_TEXT, gws::ENTRY_FIELD] {
        if style & b != 0 {
            return b;
        }
    }
    0
}

impl GameWindowManager {
    /// A manager for a display of `display` pixels.
    pub fn new(display: (i32, i32)) -> Self {
        GameWindowManager { display, ..Default::default() }
    }

    /// Records upstream's undefined behaviour (the first one is kept).
    pub fn set_undefined(&mut self, what: &'static str) {
        if self.undefined.is_none() {
            self.undefined = Some(what);
        }
    }

    /// The window `id`.
    pub fn win(&self, id: WinId) -> &GameWindow {
        &self.tree.windows[id]
    }
    /// The window `id`, mutable.
    pub fn win_mut(&mut self, id: WinId) -> &mut GameWindow {
        &mut self.tree.windows[id]
    }

    // -----------------------------------------------------------------------
    // the lists
    // -----------------------------------------------------------------------

    /// `winGetNext()`: the next window in its parent's child list, or in the window list.
    pub fn next_of(&self, w: WinId) -> Option<WinId> {
        let list = match self.tree.windows[w].parent {
            Some(p) => &self.tree.windows[p].children,
            None => &self.window_list,
        };
        let i = list.iter().position(|&x| x == w)?;
        list.get(i + 1).copied()
    }

    /// `linkWindow`: to the head of the window list, or behind the last modal window in it.
    pub fn link_window(&mut self, w: WinId) {
        let mut last_modal = None;
        for (i, &t) in self.window_list.iter().enumerate() {
            if self.modal.iter().any(|&m| m == t && m != w) {
                last_modal = Some(i);
            }
        }
        match last_modal {
            None => self.window_list.insert(0, w),
            Some(i) => self.window_list.insert(i + 1, w),
        }
    }

    /// `unlinkWindow`.
    pub fn unlink_window(&mut self, w: WinId) {
        self.window_list.retain(|&x| x != w);
    }

    /// `unlinkChildWindow`: off its parent's list, the parent cleared.
    pub fn unlink_child_window(&mut self, w: WinId) {
        if let Some(p) = self.tree.windows[w].parent {
            self.tree.windows[p].children.retain(|&x| x != w);
        }
        self.tree.windows[w].parent = None;
    }

    /// `addWindowToParent`: at the head of the parent's children.
    pub fn add_window_to_parent(&mut self, w: WinId, parent: Option<WinId>) {
        if let Some(p) = parent {
            self.tree.windows[p].children.insert(0, w);
            self.tree.windows[w].parent = Some(p);
        }
    }

    /// `addWindowToParentAtEnd`.
    pub fn add_window_to_parent_at_end(&mut self, w: WinId, parent: Option<WinId>) {
        if let Some(p) = parent {
            self.tree.windows[p].children.push(w);
            self.tree.windows[w].parent = Some(p);
        }
    }

    /// `insertWindowAheadOf(window, aheadOf)`.
    pub fn insert_window_ahead_of(&mut self, w: WinId, ahead_of: Option<WinId>) {
        let Some(a) = ahead_of else {
            self.link_window(w);
            return;
        };
        match self.tree.windows[a].parent {
            None => {
                let i = self.window_list.iter().position(|&x| x == a).unwrap_or(0);
                self.window_list.insert(i, w);
            }
            Some(p) => {
                let i = self.tree.windows[p].children.iter().position(|&x| x == a).unwrap_or(0);
                self.tree.windows[p].children.insert(i, w);
                self.tree.windows[w].parent = Some(p);
            }
        }
    }

    // -----------------------------------------------------------------------
    // state of the tree
    // -----------------------------------------------------------------------

    /// `isEnabled(win)`: it and every ancestor enabled.
    pub fn is_enabled(&self, win: Option<WinId>) -> bool {
        let Some(mut w) = win else { return false };
        loop {
            if self.tree.windows[w].status & win_status::ENABLED == 0 {
                return false;
            }
            match self.tree.windows[w].parent {
                Some(p) => w = p,
                None => return true,
            }
        }
    }

    /// `isHidden(win)`: it or an ancestor hidden (no window is hidden).
    pub fn is_hidden(&self, win: Option<WinId>) -> bool {
        let Some(mut w) = win else { return true };
        loop {
            if self.tree.windows[w].status & win_status::HIDDEN != 0 {
                return true;
            }
            match self.tree.windows[w].parent {
                Some(p) => w = p,
                None => return false,
            }
        }
    }

    /// `GameWindow::winIsChild(child)`: whether `child` is below `parent`.
    pub fn win_is_child(&self, parent: WinId, child: Option<WinId>) -> bool {
        let mut c = child;
        while let Some(w) = c {
            if self.tree.windows[w].parent == Some(parent) {
                return true;
            }
            c = self.tree.windows[w].parent;
        }
        false
    }

    /// `winEnable(enable)`: the window and every descendant.
    pub fn win_enable(&mut self, w: WinId, enable: bool) {
        if enable {
            self.tree.windows[w].status |= win_status::ENABLED;
        } else {
            self.tree.windows[w].status &= !win_status::ENABLED;
        }
        for c in self.tree.windows[w].children.clone() {
            self.win_enable(c, enable);
        }
    }

    /// `winHide(hide)`; hiding tells the manager (`windowHiding`). (`freeImages` has nothing to free here.)
    pub fn win_hide(&mut self, w: WinId, hide: bool) {
        if hide {
            self.tree.windows[w].status |= win_status::HIDDEN;
            self.window_hiding(w);
        } else {
            self.tree.windows[w].status &= !win_status::HIDDEN;
        }
    }

    /// `windowHiding`: a hidden window loses the focus, the modal top and the capture; so do its children.
    pub fn window_hiding(&mut self, w: WinId) {
        if self.keyboard_focus == Some(w) {
            self.keyboard_focus = None;
        }
        if self.modal.first() == Some(&w) {
            self.win_unset_modal(Some(w));
        }
        if self.mouse_captor == Some(w) {
            self.win_capture(None);
        }
        for c in self.tree.windows[w].children.clone() {
            self.window_hiding(c);
        }
    }

    /// `winSetStatus(bits)`: sets them, returns the old status.
    pub fn win_set_status(&mut self, w: WinId, bits: u32) -> u32 {
        let old = self.tree.windows[w].status;
        self.tree.windows[w].status |= bits;
        old
    }

    /// `winClearStatus(bits)`.
    pub fn win_clear_status(&mut self, w: WinId, bits: u32) -> u32 {
        let old = self.tree.windows[w].status;
        self.tree.windows[w].status &= !bits;
        old
    }

    /// `winSetHiliteState`.
    pub fn win_set_hilite_state(&mut self, w: WinId, state: bool) {
        if state {
            self.tree.windows[w].inst.state |= WIN_STATE_HILITED;
        } else {
            self.tree.windows[w].inst.state &= !WIN_STATE_HILITED;
        }
    }

    /// `winSetPosition(x, y)`.
    pub fn win_set_position(&mut self, w: WinId, x: i32, y: i32) {
        let win = &mut self.tree.windows[w];
        win.region = (x, y, x.wrapping_add(win.size.0), y.wrapping_add(win.size.1));
        normalize_window_region(win);
    }

    /// `winSetSize(w, h)`: then `GGM_RESIZED` to the window.
    pub fn win_set_size<H: GuiHost>(&mut self, h: &mut H, w: WinId, width: i32, height: i32) {
        let win = &mut self.tree.windows[w];
        win.size = (width, height);
        win.region.2 = win.region.0.wrapping_add(width);
        win.region.3 = win.region.1.wrapping_add(height);
        self.win_send_system_msg(h, Some(w), ggm::RESIZED, width as u32, height as u32);
    }

    /// `winPointInWindow(x, y)`: inside the window's screen rectangle, edges included.
    pub fn win_point_in_window(&self, w: WinId, x: i32, y: i32) -> bool {
        let (wx, wy) = self.tree.screen_position(w);
        let (sw, sh) = self.tree.windows[w].size;
        x >= wx && x <= wx + sw && y >= wy && y <= wy + sh
    }

    /// `winSetOwner(owner)`: `NULL` makes the window its own owner.
    pub fn win_set_owner(&mut self, w: WinId, owner: Option<WinId>) {
        self.ext[w].owner = Some(owner.unwrap_or(w));
    }

    /// `winGetOwner()`.
    pub fn win_get_owner(&self, w: WinId) -> Option<WinId> {
        self.ext[w].owner
    }

    /// `winSetParent(parent)`.
    pub fn win_set_parent(&mut self, w: WinId, parent: Option<WinId>) {
        if self.tree.windows[w].parent.is_none() {
            self.unlink_window(w);
        } else {
            self.unlink_child_window(w);
        }
        match parent {
            None => {
                self.link_window(w);
                self.tree.windows[w].parent = None;
            }
            Some(p) => self.add_window_to_parent(w, Some(p)),
        }
    }

    /// `winBringToTop()`: the head of its parent's children, or of the window list (behind modal windows); its layout
    /// moves it to the layout's head too.
    pub fn win_bring_to_top(&mut self, w: WinId) -> i32 {
        if let Some(p) = self.tree.windows[w].parent {
            self.unlink_child_window(w);
            self.add_window_to_parent(w, Some(p));
        } else {
            if !self.window_list.contains(&w) {
                return WIN_ERR_INVALID_PARAMETER;
            }
            self.unlink_window(w);
            self.link_window(w);
        }
        if let Some(l) = self.ext[w].layout {
            self.layout_remove_window(l, w);
            self.layout_add_window(l, w);
        }
        WIN_ERR_OK
    }

    /// `winActivate()`: to the top, active, shown.
    pub fn win_activate(&mut self, w: WinId) -> i32 {
        let r = self.win_bring_to_top(w);
        if r != WIN_ERR_OK {
            return r;
        }
        self.tree.windows[w].status |= win_status::ACTIVE;
        self.win_hide(w, false);
        WIN_ERR_OK
    }

    /// `winGetWindowFromId(window, id)`: depth first from `window` (or the window list) along its siblings.
    pub fn win_get_window_from_id(&self, window: Option<WinId>, id: i32) -> Option<WinId> {
        let list: Vec<WinId> = match window {
            None => self.window_list.clone(),
            Some(w) => {
                let sibs = match self.tree.windows[w].parent {
                    Some(p) => &self.tree.windows[p].children,
                    None => &self.window_list,
                };
                let i = sibs.iter().position(|&x| x == w).unwrap_or(0);
                sibs[i..].to_vec()
            }
        };
        for w in list {
            if self.tree.windows[w].id == id {
                return Some(w);
            }
            if let Some(&c) = self.tree.windows[w].children.first() {
                if let Some(f) = self.win_get_window_from_id(Some(c), id) {
                    return Some(f);
                }
            }
        }
        None
    }

    /// `hideWindowsInRange(base, first, last, hide)`.
    pub fn hide_windows_in_range(&mut self, base: Option<WinId>, first: i32, last: i32, hide: bool) {
        for i in first..=last {
            if let Some(w) = self.win_get_window_from_id(base, i) {
                self.win_hide(w, hide);
            }
        }
    }

    /// `enableWindowsInRange(base, first, last, enable)`.
    pub fn enable_windows_in_range(&mut self, base: Option<WinId>, first: i32, last: i32, enable: bool) {
        for i in first..=last {
            if let Some(w) = self.win_get_window_from_id(base, i) {
                self.win_enable(w, enable);
            }
        }
    }

    // -----------------------------------------------------------------------
    // capture, focus, modal, grab, lone window, tabs
    // -----------------------------------------------------------------------

    /// `winCapture(window)`: fails while another window holds the capture.
    pub fn win_capture(&mut self, w: Option<WinId>) -> i32 {
        if self.mouse_captor.is_some() {
            return WIN_ERR_MOUSE_CAPTURED;
        }
        self.mouse_captor = w;
        WIN_ERR_OK
    }

    /// `winRelease(window)`.
    pub fn win_release(&mut self, w: Option<WinId>) -> i32 {
        if w == self.mouse_captor {
            self.mouse_captor = None;
        }
        WIN_ERR_OK
    }

    /// `winSetFocus(window)` (see the module docs).
    pub fn win_set_focus<H: GuiHost>(&mut self, h: &mut H, window: Option<WinId>) -> i32 {
        let mut wants = false;
        if let Some(w) = window {
            if self.tree.windows[w].status & win_status::NO_FOCUS != 0 {
                return 0;
            }
        }
        if let Some(old) = self.keyboard_focus {
            if Some(old) != window {
                // `Bool wf; winSendSystemMsg(m_keyboardFocus, GWM_INPUT_FOCUS, FALSE, &wf)`: the answer is ignored
                self.focus_reply = false;
                self.win_send_system_msg(h, Some(old), gwm::INPUT_FOCUS, 0, 1);
            }
        }
        self.keyboard_focus = window;
        if self.keyboard_focus.is_some() {
            let mut w = window;
            while let Some(cur) = w {
                self.focus_reply = wants;
                self.win_send_system_msg(h, Some(cur), gwm::INPUT_FOCUS, 1, 1);
                wants = self.focus_reply;
                if wants {
                    break;
                }
                w = self.tree.windows[cur].parent;
            }
        }
        if !wants {
            self.keyboard_focus = None;
        }
        WIN_ERR_OK
    }

    /// `winSetModal(window)`: a root window only, to the top of the modal stack.
    pub fn win_set_modal(&mut self, w: Option<WinId>) -> i32 {
        let Some(w) = w else { return WIN_ERR_INVALID_WINDOW };
        if self.tree.windows[w].parent.is_some() {
            return WIN_ERR_INVALID_PARAMETER;
        }
        self.modal.insert(0, w);
        WIN_ERR_OK
    }

    /// `winUnsetModal(window)`: only the top of the stack.
    pub fn win_unset_modal(&mut self, w: Option<WinId>) -> i32 {
        let Some(w) = w else { return WIN_ERR_INVALID_WINDOW };
        if self.modal.first() != Some(&w) {
            return WIN_ERR_GENERAL_FAILURE;
        }
        self.modal.remove(0);
        WIN_ERR_OK
    }

    /// `winSetLoneWindow(window)`: the previous lone window gets `GGM_CLOSE`.
    pub fn win_set_lone_window<H: GuiHost>(&mut self, h: &mut H, w: Option<WinId>) {
        if self.lone_window == w {
            return;
        }
        if let Some(old) = self.lone_window {
            self.win_send_system_msg(h, Some(old), ggm::CLOSE, 0, 0);
        }
        self.lone_window = w;
    }

    /// `registerTabList`.
    pub fn register_tab_list(&mut self, list: Vec<WinId>) {
        self.tab_list = list;
    }

    /// `clearTabList`.
    pub fn clear_tab_list(&mut self) {
        self.tab_list.clear();
    }

    /// `GameWindowManager::winNextTab(window)`: the focus to the next window of the tab list (wrapping).
    pub fn win_next_tab<H: GuiHost>(&mut self, h: &mut H, w: WinId) {
        if self.tab_list.is_empty() || !self.modal.is_empty() {
            return;
        }
        let i = self.tab_list.iter().position(|&x| x == w).map_or(self.tab_list.len(), |i| i + 1);
        let to = self.tab_list.get(i).copied().unwrap_or(self.tab_list[0]);
        self.win_set_focus(h, Some(to));
        self.win_set_lone_window(h, None);
    }

    /// `GameWindowManager::winPrevTab(window)`.
    pub fn win_prev_tab<H: GuiHost>(&mut self, h: &mut H, w: WinId) {
        if self.tab_list.is_empty() || !self.modal.is_empty() {
            return;
        }
        let n = self.tab_list.len();
        let to = match self.tab_list.iter().position(|&x| x == w) {
            Some(i) if i > 0 => self.tab_list[i - 1],
            _ => self.tab_list[n - 1],
        };
        self.win_set_focus(h, Some(to));
        self.win_set_lone_window(h, None);
    }

    // -----------------------------------------------------------------------
    // messages
    // -----------------------------------------------------------------------

    /// `winSendSystemMsg`.
    pub fn win_send_system_msg<H: GuiHost>(&mut self, h: &mut H, w: Option<WinId>, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        let Some(w) = w else { return MsgHandled::Ignored };
        if msg != gwm::DESTROY && self.tree.windows[w].status & win_status::DESTROYED != 0 {
            return MsgHandled::Ignored;
        }
        match self.ext[w].system.clone() {
            SystemFunc::Default => MsgHandled::Ignored,
            SystemFunc::PassSelectedButtonsToParent => {
                if matches!(msg, ggm::GBM_SELECTED | ggm::GBM_SELECTED_RIGHT | ggm::GBM_MOUSE_ENTERING | ggm::GBM_MOUSE_LEAVING | ggm::GEM_EDIT_DONE) {
                    if let Some(p) = self.tree.windows[w].parent {
                        return self.win_send_system_msg(h, Some(p), msg, d1, d2);
                    }
                }
                MsgHandled::Ignored
            }
            SystemFunc::PassMessagesToParent => match self.tree.windows[w].parent {
                Some(p) => self.win_send_system_msg(h, Some(p), msg, d1, d2),
                None => MsgHandled::Ignored,
            },
            SystemFunc::PushButton => crate::client::gadget::push_button_system(self, h, w, msg, d1, d2),
            SystemFunc::CheckBox => crate::client::gadget::check_box_system(self, h, w, msg, d1, d2),
            SystemFunc::RadioButton => crate::client::gadget::radio_button_system(self, h, w, msg, d1, d2),
            SystemFunc::StaticText => crate::client::gadget::static_text_system(self, h, w, msg, d1, d2),
            SystemFunc::ProgressBar => crate::client::gadget::progress_bar_system(self, h, w, msg, d1, d2),
            SystemFunc::ListBox => crate::client::gadget_list_box::list_box_system(self, h, w, msg, d1, d2),
            SystemFunc::VerticalSlider => crate::client::gadget_slider::vertical_slider_system(self, h, w, msg, d1, d2),
            SystemFunc::HorizontalSlider => crate::client::gadget_slider::horizontal_slider_system(self, h, w, msg, d1, d2),
            SystemFunc::TextEntry => crate::client::gadget_text_entry::text_entry_system(self, h, w, msg, d1, d2),
            SystemFunc::ComboBox => crate::client::gadget_combo_box::combo_box_system(self, h, w, msg, d1, d2),
            SystemFunc::TabControl => crate::client::gadget_tab_control::tab_control_system(self, h, w, msg, d1, d2),
            SystemFunc::Host(name) => h.system(self, &name, w, msg, d1, d2),
        }
    }

    /// `winSendInputMsg`.
    pub fn win_send_input_msg<H: GuiHost>(&mut self, h: &mut H, w: Option<WinId>, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        let Some(w) = w else { return MsgHandled::Ignored };
        if msg != gwm::DESTROY && self.tree.windows[w].status & win_status::DESTROYED != 0 {
            return MsgHandled::Ignored;
        }
        match self.ext[w].input.clone() {
            InputFunc::Default => MsgHandled::Ignored,
            InputFunc::Block => {
                // `GameWinBlockInput`
                if msg == gwm::CHAR || msg == gwm::MOUSE_POS {
                    return MsgHandled::Ignored;
                }
                if msg == gwm::LEFT_UP {
                    h.block_input_left_up();
                }
                MsgHandled::Handled
            }
            InputFunc::PushButton => crate::client::gadget::push_button_input(self, h, w, msg, d1, d2),
            InputFunc::CheckBox => crate::client::gadget::check_box_input(self, h, w, msg, d1, d2),
            InputFunc::RadioButton => crate::client::gadget::radio_button_input(self, h, w, msg, d1, d2),
            InputFunc::StaticText => crate::client::gadget::static_text_input(self, h, w, msg, d1, d2),
            InputFunc::ListBox => crate::client::gadget_list_box::list_box_input(self, h, w, msg, d1, d2),
            InputFunc::ListBoxMulti => crate::client::gadget_list_box::list_box_multi_input(self, h, w, msg, d1, d2),
            InputFunc::VerticalSlider => crate::client::gadget_slider::vertical_slider_input(self, h, w, msg, d1, d2),
            InputFunc::HorizontalSlider => crate::client::gadget_slider::horizontal_slider_input(self, h, w, msg, d1, d2),
            InputFunc::TextEntry => crate::client::gadget_text_entry::text_entry_input(self, h, w, msg, d1, d2),
            InputFunc::ComboBox => crate::client::gadget_combo_box::combo_box_input(self, h, w, msg, d1, d2),
            InputFunc::TabControl => crate::client::gadget_tab_control::tab_control_input(self, h, w, msg, d1, d2),
            InputFunc::Host(name) => h.input(self, &name, w, msg, d1, d2),
        }
    }

    /// `winProcessKey(key, state)`: to the focus window and up its parents.
    pub fn win_process_key<H: GuiHost>(&mut self, h: &mut H, k: u8, state: u32) -> WinInputReturnCode {
        let mut rc = WinInputReturnCode::NotUsed;
        if let Some(f) = self.keyboard_focus {
            if k != key::NONE {
                let mut w = f;
                rc = WinInputReturnCode::Used;
                while self.win_send_input_msg(h, Some(w), gwm::CHAR, u32::from(k), state) == MsgHandled::Ignored {
                    match self.tree.windows[w].parent {
                        Some(p) => w = p,
                        None => {
                            rc = WinInputReturnCode::NotUsed;
                            break;
                        }
                    }
                }
            }
        }
        rc
    }

    /// The screen origin of `child` (its region's corner plus its ancestors').
    fn origin(&self, child: WinId) -> (i32, i32) {
        self.tree.screen_position(child)
    }

    /// `GameWindow::winPointInChild(x, y, ignoreEnableCheck, playDisabledSound)`.
    pub fn win_point_in_child<H: GuiHost>(&self, h: &mut H, w: WinId, x: i32, y: i32, ignore_enable: bool, play_disabled: bool) -> WinId {
        for &c in &self.tree.windows[w].children {
            let (ox, oy) = self.origin(c);
            let cw = &self.tree.windows[c];
            if x >= ox && x <= ox + cw.size.0 && y >= oy && y <= oy + cw.size.1 {
                let enabled = ignore_enable || cw.status & win_status::ENABLED != 0;
                let hidden = cw.status & win_status::HIDDEN != 0;
                if !hidden {
                    if enabled {
                        return self.win_point_in_child(h, c, x, y, ignore_enable, play_disabled);
                    } else if play_disabled {
                        h.add_audio_event(b"GUIClickDisabled");
                    }
                }
            }
        }
        w
    }

    /// `GameWindow::winPointInAnyChild(x, y, ignoreHidden, ignoreEnableCheck)`.
    pub fn win_point_in_any_child<H: GuiHost>(&self, h: &mut H, w: WinId, x: i32, y: i32, ignore_hidden: bool, ignore_enable: bool) -> WinId {
        for &c in &self.tree.windows[w].children {
            let (ox, oy) = self.origin(c);
            let cw = &self.tree.windows[c];
            if x >= ox && x <= ox + cw.size.0 && y >= oy && y <= oy + cw.size.1 && !(ignore_hidden && cw.status & win_status::HIDDEN != 0) {
                return self.win_point_in_child(h, c, x, y, ignore_enable, false);
            }
        }
        w
    }

    /// Whether a top-level window's region holds the point.
    fn in_region(&self, w: WinId, x: i32, y: i32) -> bool {
        let r = self.tree.windows[w].region;
        x >= r.0 && x <= r.2 && y >= r.1 && y <= r.3
    }

    /// Whether a window offers a tooltip (`m_tooltip || getTooltipTextLength()`).
    fn has_tooltip(&self, w: WinId) -> bool {
        self.ext[w].tooltip.is_some() || self.tree.windows[w].inst.tooltip_text.as_ref().is_some_and(|t| !t.is_empty())
    }

    /// `winProcessMouseEvent(msg, mousePos, data)`; `delta` is the `ICoord2D` a drag passes as `data`.
    pub fn win_process_mouse_event<H: GuiHost>(&mut self, h: &mut H, msg: u32, pos: (i32, i32), delta: (i32, i32)) -> WinInputReturnCode {
        let (x, y) = pos;
        let mut rc = WinInputReturnCode::NotUsed;
        let packed = short_to_long(x, y);
        let mut window: Option<WinId> = None;
        let mut tool: Option<WinId> = None;
        let mut clear_grab = false;
        self.mouse.set_cursor_tooltip(&[], -1);
        if let Some(cap) = self.mouse_captor {
            self.grab_window = None;
            window = Some(self.win_point_in_child(h, cap, x, y, false, false));
            if SEND_MOUSE_POS_MESSAGES || msg != gwm::MOUSE_POS {
                let mut win = window;
                while let Some(w) = win {
                    if self.win_send_input_msg(h, Some(w), msg, packed, 0) == MsgHandled::Handled {
                        rc = WinInputReturnCode::Used;
                        break;
                    }
                    if Some(w) == self.mouse_captor {
                        break;
                    }
                    win = self.tree.windows[w].parent;
                }
            }
        } else if let Some(g) = self.grab_window {
            match msg {
                gwm::LEFT_UP => {
                    // "Play a beep sound if the window is disabled."
                    self.win_point_in_child(h, g, x, y, false, true);
                    self.tree.windows[g].status &= !win_status::ACTIVE;
                    if self.win_point_in_window(g, x, y) || self.tree.windows[g].status & win_status::DRAGABLE != 0 {
                        self.win_send_input_msg(h, Some(g), gwm::LEFT_UP, packed, 0);
                    }
                    clear_grab = true;
                }
                gwm::NONE | gwm::LEFT_DRAG => {
                    if self.tree.windows[g].status & win_status::DRAGABLE != 0 {
                        let (mut dx, mut dy) = delta;
                        let gw = &self.tree.windows[g];
                        if let Some(p) = gw.parent {
                            let ps = self.tree.windows[p].size;
                            if gw.region.0 + dx < 0 {
                                dx = -gw.region.0;
                            } else if gw.region.2 + dx > ps.0 {
                                dx = ps.0 - gw.region.2;
                            }
                            if gw.region.1 + dy < 0 {
                                dy = -gw.region.1;
                            } else if gw.region.3 + dy > ps.1 {
                                dy = ps.1 - gw.region.3;
                            }
                        }
                        let size = gw.size;
                        let (mut lx, mut ly) = (gw.region.0 + dx, gw.region.1 + dy);
                        lx = lx.max(0);
                        ly = ly.max(0);
                        let hx = (lx + size.0).min(self.display.0);
                        let hy = (ly + size.1).min(self.display.1);
                        self.win_set_position(g, hx - size.0, hy - size.1);
                    }
                    self.win_send_input_msg(h, Some(g), msg, packed, 0);
                }
                _ => {}
            }
            rc = WinInputReturnCode::Used;
        } else {
            if let Some(&m) = self.modal.first() {
                // `winDestroy` unsets only the top of the modal stack: a destroyed window below it stays, and once it
                // is freed and the windows above are unset, upstream searches freed memory (#72)
                if self.ext[m].freed {
                    self.set_undefined("winProcessMouseEvent: the modal head is a freed window (destroyed below the top of the modal stack)");
                    return WinInputReturnCode::NotUsed;
                }
                window = Some(self.win_point_in_child(h, m, x, y, false, false));
            } else {
                let list = self.window_list.clone();
                let passes: [fn(u32) -> bool; 3] = [
                    |s| s & win_status::ABOVE != 0 && s & win_status::HIDDEN == 0,
                    |s| s & (win_status::ABOVE | win_status::BELOW | win_status::HIDDEN) == 0,
                    |s| s & win_status::BELOW != 0 && s & win_status::HIDDEN == 0,
                ];
                'passes: for pass in passes {
                    for &w in &list {
                        let s = self.tree.windows[w].status;
                        if pass(s) && self.in_region(w, x, y) {
                            let child = self.win_point_in_any_child(h, w, x, y, true, true);
                            if tool.is_none() && self.has_tooltip(child) {
                                tool = Some(child);
                            }
                            if s & win_status::ENABLED != 0 {
                                window = Some(self.win_point_in_child(h, w, x, y, false, false));
                                break 'passes;
                            }
                        }
                    }
                }
            }
            if let Some(w) = window {
                if self.tree.windows[w].status & win_status::NO_INPUT != 0 {
                    window = match self.tree.windows[w].parent {
                        Some(p) if self.tree.windows[p].inst.style & gws::COMBO_BOX != 0 => Some(p),
                        _ => None,
                    };
                }
            }
            if let Some(w) = window {
                if SEND_MOUSE_POS_MESSAGES || msg != gwm::MOUSE_POS {
                    let mut temp = Some(w);
                    let old_lone = self.lone_window;
                    while let Some(t) = temp {
                        if self.win_send_input_msg(h, Some(t), msg, packed, 0) != MsgHandled::Ignored {
                            break;
                        }
                        temp = self.tree.windows[t].parent;
                    }
                    if let Some(lone) = self.lone_window {
                        if Some(lone) == old_lone && (msg == gwm::LEFT_UP || msg == gwm::MIDDLE_UP || msg == gwm::RIGHT_UP || temp.is_some()) && !self.win_is_child(lone, temp) {
                            self.win_set_lone_window(h, None);
                        }
                    }
                    if let Some(t) = temp {
                        if msg == gwm::LEFT_DOWN {
                            self.grab_window = Some(t);
                        }
                        rc = WinInputReturnCode::Used;
                    }
                }
            }
            if tool.is_none() && !self.is_hidden(window) {
                tool = window;
            }
            match tool {
                Some(t) => {
                    if let Some(name) = self.ext[t].tooltip.clone() {
                        h.tooltip(self, &name, t, packed);
                    } else if let Some(text) = self.tree.windows[t].inst.tooltip_text.clone().filter(|t| !t.is_empty()) {
                        let d = self.tree.windows[t].inst.tooltip_delay;
                        self.mouse.set_cursor_tooltip(&text, d);
                    }
                }
                None => {
                    // `objectTooltip = TRUE`: the world's objects may set one (not modelled here)
                }
            }
        }
        if self.grab_window.is_none() && window != self.curr_mouse_rgn {
            if let Some(cap) = self.mouse_captor {
                if self.win_is_child(cap, self.curr_mouse_rgn) {
                    self.win_send_input_msg(h, self.curr_mouse_rgn, gwm::MOUSE_LEAVING, packed, 0);
                }
            } else if self.curr_mouse_rgn.is_some() {
                self.win_send_input_msg(h, self.curr_mouse_rgn, gwm::MOUSE_LEAVING, packed, 0);
            }
            if window.is_some() {
                self.win_send_input_msg(h, window, gwm::MOUSE_ENTERING, packed, 0);
            }
            self.curr_mouse_rgn = window;
        }
        if clear_grab {
            self.grab_window = None;
        }
        rc
    }

    /// `getWindowUnderCursor(x, y, ignoreEnabled)`.
    pub fn get_window_under_cursor<H: GuiHost>(&self, h: &mut H, x: i32, y: i32, ignore_enabled: bool) -> Option<WinId> {
        if let Some(c) = self.mouse_captor {
            return Some(self.win_point_in_child(h, c, x, y, ignore_enabled, false));
        }
        if let Some(g) = self.grab_window {
            return Some(self.win_point_in_child(h, g, x, y, ignore_enabled, false));
        }
        if let Some(&m) = self.modal.first() {
            return Some(self.win_point_in_child(h, m, x, y, ignore_enabled, false));
        }
        let mut window = None;
        let passes: [fn(u32) -> bool; 3] = [
            |s| s & win_status::ABOVE != 0 && s & win_status::HIDDEN == 0,
            |s| s & (win_status::ABOVE | win_status::BELOW | win_status::HIDDEN) == 0,
            |s| s & win_status::BELOW != 0 && s & win_status::HIDDEN == 0,
        ];
        'passes: for pass in passes {
            for &w in &self.window_list {
                let s = self.tree.windows[w].status;
                if pass(s) && self.in_region(w, x, y) && (s & win_status::ENABLED != 0 || ignore_enabled) {
                    window = Some(self.win_point_in_child(h, w, x, y, ignore_enabled, false));
                    break 'passes;
                }
            }
        }
        if let Some(w) = window {
            let s = self.tree.windows[w].status;
            if s & win_status::NO_INPUT != 0 || (ignore_enabled && s & win_status::ENABLED == 0) {
                window = None;
            }
        }
        window
    }

    // -----------------------------------------------------------------------
    // creation and destruction
    // -----------------------------------------------------------------------

    /// `winCreate(parent, status, x, y, width, height, system, instData)`: the window's id. The default font
    /// replaces the instance data's (`TheGlobalLanguageData` is not modelled: Times New Roman 14).
    #[allow(clippy::too_many_arguments)]
    pub fn win_create<H: GuiHost>(&mut self, h: &mut H, parent: Option<WinId>, status: u32, x: i32, y: i32, width: i32, height: i32, system: SystemFunc, inst: Option<WinInstanceData>) -> WinId {
        let id = self.tree.windows.len();
        let mut win = GameWindow {
            parent: None,
            children: Vec::new(),
            window_type: Vec::new(),
            status,
            size: (width, height),
            region: (x, y, x.wrapping_add(width), y.wrapping_add(height)),
            id: 0,
            inst: WinInstanceData::new(),
            gadget: GadgetData::None,
            parts: Vec::new(),
            text: None,
            callbacks: [None, None, None, None],
        };
        normalize_window_region(&mut win);
        self.tree.windows.push(win);
        self.ext.push(WinExt::default());
        if parent.is_some() {
            self.add_window_to_parent(id, parent);
        } else {
            self.link_window(id);
        }
        self.ext[id].system = system;
        self.win_send_system_msg(h, Some(id), gwm::CREATE, 0, 0);
        if let Some(i) = inst {
            self.tree.windows[id].inst = i;
        }
        self.tree.windows[id].inst.font = Some(default_font());
        id
    }

    /// The gadget part of `gogoGadget*` after `winCreate`: the input and draw callbacks (the image draw with
    /// `WIN_STATUS_IMAGE`), the owner, the user data. (`assignDefaultGadgetLook` with `defaultVisual` FALSE, as the
    /// script asks, only sets the font, which the window has.)
    fn install_gadget(&mut self, w: WinId, parent: Option<WinId>) {
        let style = self.tree.windows[w].inst.style;
        let image = self.tree.windows[w].status & win_status::IMAGE != 0;
        let pick = |a: DrawFunc, b: DrawFunc| if image { a } else { b };
        let e = &mut self.ext[w];
        match gadget_style(style) {
            gws::PUSH_BUTTON => {
                e.input = InputFunc::PushButton;
                e.draw = pick(DrawFunc::PushButtonImage, DrawFunc::PushButton);
                e.user = UserData::PushButton(None);
            }
            gws::CHECK_BOX => {
                e.input = InputFunc::CheckBox;
                e.draw = pick(DrawFunc::CheckBoxImage, DrawFunc::CheckBox);
            }
            gws::RADIO_BUTTON => {
                e.input = InputFunc::RadioButton;
                e.draw = pick(DrawFunc::RadioButtonImage, DrawFunc::RadioButton);
                let r = match &self.tree.windows[w].gadget {
                    GadgetData::Radio(r) => *r,
                    _ => RadioButtonData::default(),
                };
                e.user = UserData::Radio(r);
            }
            gws::STATIC_TEXT => {
                e.input = InputFunc::StaticText;
                e.draw = pick(DrawFunc::StaticTextImage, DrawFunc::StaticText);
                let t = match &self.tree.windows[w].gadget {
                    GadgetData::Text(t) => *t,
                    _ => TextData::default(),
                };
                e.user = UserData::StaticText(t, Vec::new());
            }
            gws::PROGRESS_BAR => {
                e.draw = pick(DrawFunc::ProgressBarImage, DrawFunc::ProgressBar);
                e.user = UserData::Progress(0);
            }
            _ => {}
        }
        self.win_set_owner(w, parent);
    }

    /// The system callback `winCreate` gets for a gadget of this style.
    fn gadget_system(style: u32) -> Option<SystemFunc> {
        Some(match gadget_style(style) {
            gws::PUSH_BUTTON => SystemFunc::PushButton,
            gws::CHECK_BOX => SystemFunc::CheckBox,
            gws::RADIO_BUTTON => SystemFunc::RadioButton,
            gws::STATIC_TEXT => SystemFunc::StaticText,
            gws::PROGRESS_BAR => SystemFunc::ProgressBar,
            gws::TAB_CONTROL => SystemFunc::TabControl,
            gws::VERT_SLIDER => SystemFunc::VerticalSlider,
            gws::HORZ_SLIDER => SystemFunc::HorizontalSlider,
            gws::SCROLL_LISTBOX => SystemFunc::ListBox,
            gws::COMBO_BOX => SystemFunc::ComboBox,
            gws::ENTRY_FIELD => SystemFunc::TextEntry,
            _ => return None,
        })
    }

    /// The slider's input and draw callbacks (`gogoGadgetSlider`): the image draw with `WIN_STATUS_IMAGE`, except
    /// for a vertical slider in a list box.
    pub(crate) fn install_slider_callbacks(&mut self, s: WinId, parent: Option<WinId>) {
        let horz = self.tree.windows[s].inst.style & gws::HORZ_SLIDER != 0;
        let image = self.tree.windows[s].status & win_status::IMAGE != 0;
        let in_listbox = parent.is_some_and(|p| self.tree.windows[p].inst.style & gws::SCROLL_LISTBOX != 0);
        let e = &mut self.ext[s];
        if horz {
            e.input = InputFunc::HorizontalSlider;
            e.draw = if image { DrawFunc::HorizontalSliderImage } else { DrawFunc::HorizontalSlider };
        } else {
            e.input = InputFunc::VerticalSlider;
            e.draw = if image && !in_listbox { DrawFunc::VerticalSliderImage } else { DrawFunc::VerticalSlider };
        }
    }

    /// `GameWindow::winSetFont(font)`: the window's font, and the special gadgets' strings (a list box's cells, a combo
    /// box's list box and edit box, a text entry's strings; the other display strings take the window's font when
    /// drawn).
    pub fn win_set_font(&mut self, w: WinId, font: Option<GameFont>) {
        self.tree.windows[w].inst.font = font.clone();
        let style = self.tree.windows[w].inst.style;
        if style & gws::SCROLL_LISTBOX != 0 {
            crate::client::gadget_list_box::set_font(self, w, font);
        } else if style & gws::COMBO_BOX != 0 {
            crate::client::gadget_combo_box::set_font(self, w, font);
        } else if style & gws::ENTRY_FIELD != 0 {
            crate::client::gadget_text_entry::set_font(self, w, font);
        }
    }

    /// `GameWindow::winSet{Enabled,Disabled,Hilite}TextColors(color, borderColor)` (`which` 0, 1, 2): a combo box's
    /// also go to its list box and edit box.
    pub fn win_set_text_colors(&mut self, w: WinId, which: usize, color: i32, border: i32) {
        let inst = &mut self.tree.windows[w].inst;
        let t = match which {
            0 => &mut inst.enabled_text,
            1 => &mut inst.disabled_text,
            _ => &mut inst.hilite_text,
        };
        t.color = color;
        t.border_color = border;
        if inst.style & gws::COMBO_BOX != 0 {
            crate::client::gadget_combo_box::set_text_colors(self, w, which, color, border);
        }
    }

    /// `gogoGadgetPushButton(parent, status, x, y, w, h, instData, NULL, defaultVisual)` for an inner button (no
    /// label); the gadgets ask for the default visual (`TRUE`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn gogo_push_button<H: GuiHost>(&mut self, h: &mut H, parent: Option<WinId>, status: u32, x: i32, y: i32, width: i32, height: i32, inst: WinInstanceData, default_visual: bool) -> WinId {
        self.gogo_gadget_look(h, parent, status, x, y, width, height, inst, GadgetData::None, default_visual).expect("a push button")
    }

    /// [`Self::gogo_gadget_look`] without the default visual (`defaultVisual` FALSE, as the scripts make gadgets).
    #[allow(clippy::too_many_arguments)]
    pub fn gogo_gadget<H: GuiHost>(&mut self, h: &mut H, parent: Option<WinId>, status: u32, x: i32, y: i32, width: i32, height: i32, inst: WinInstanceData, data: GadgetData) -> Option<WinId> {
        self.gogo_gadget_look(h, parent, status, x, y, width, height, inst, data, false)
    }

    /// `gogoGadgetPushButton`, `gogoGadgetCheckbox`, `gogoGadgetRadioButton`, `gogoGadgetStaticText`,
    /// `gogoGadgetProgressBar`, `gogoGadgetListBox`, `gogoGadgetSlider`, `gogoGadgetTextEntry`, `gogoGadgetComboBox`,
    /// `gogoGadgetTabControl` (by the instance data's style; `defaultFont` is the instance data's font), with the label
    /// as its text (`winTextLabelToText`: the label translated byte for byte). `None` where upstream asserts (no
    /// gadget style).
    #[allow(clippy::too_many_arguments)]
    pub fn gogo_gadget_look<H: GuiHost>(&mut self, h: &mut H, parent: Option<WinId>, status: u32, x: i32, y: i32, width: i32, height: i32, mut inst: WinInstanceData, data: GadgetData, default_visual: bool) -> Option<WinId> {
        let style = gadget_style(inst.style);
        match style {
            gws::SCROLL_LISTBOX => {
                let t = match &data {
                    GadgetData::Listbox(d) => crate::client::gadget_list_box::listbox_template(d),
                    _ => crate::client::gadget_list_box::ListboxRt::default(),
                };
                return crate::client::gadget_list_box::gogo_gadget_list_box_look(self, h, parent, status, x, y, width, height, inst, t, default_visual);
            }
            gws::VERT_SLIDER | gws::HORZ_SLIDER => {
                let mut t = match &data {
                    GadgetData::Slider(d) => crate::client::gadget_slider::slider_template(d),
                    _ => crate::client::gadget_slider::SliderRt::default(),
                };
                return crate::client::gadget_slider::gogo_gadget_slider(self, h, parent, status, x, y, width, height, inst, &mut t, default_visual);
            }
            gws::ENTRY_FIELD => {
                let t = match &data {
                    GadgetData::Entry(d) => crate::client::gadget_text_entry::entry_template(d),
                    _ => crate::client::gadget_text_entry::EntryRt::default(),
                };
                return crate::client::gadget_text_entry::gogo_gadget_text_entry(self, h, parent, status, x, y, width, height, inst, t, default_visual);
            }
            gws::COMBO_BOX => {
                let t = match &data {
                    GadgetData::ComboBox(d) => crate::client::gadget_combo_box::combo_template(d),
                    _ => crate::client::gadget_combo_box::combo_template(&Default::default()),
                };
                return crate::client::gadget_combo_box::gogo_gadget_combo_box(self, h, parent, status, x, y, width, height, inst, t, default_visual);
            }
            gws::TAB_CONTROL => {
                let t = match &data {
                    GadgetData::TabControl(d) => crate::client::gadget_tab_control::tab_template(d),
                    _ => crate::client::gadget_tab_control::TabRt::default(),
                };
                return crate::client::gadget_tab_control::gogo_gadget_tab_control(self, h, parent, status, x, y, width, height, inst, t, default_visual);
            }
            gws::PUSH_BUTTON | gws::CHECK_BOX | gws::RADIO_BUTTON | gws::STATIC_TEXT | gws::PROGRESS_BAR => {}
            _ => return None,
        }
        if style == gws::STATIC_TEXT {
            inst.style &= !gws::TAB_STOP;
        }
        let label: Vec<u16> = inst.text_label_string.iter().map(|&b| b as i8 as i16 as u16).collect();
        let font = inst.font.clone();
        let w = self.win_create(h, parent, status, x, y, width, height, Self::gadget_system(inst.style)?, Some(inst));
        self.tree.windows[w].gadget = data;
        self.install_gadget(w, parent);
        crate::client::gadget_look::assign_default_gadget_look(self, h, w, font, default_visual);
        if !label.is_empty() && style != gws::PROGRESS_BAR {
            self.set_label(h, w, &label);
        }
        Some(w)
    }

    /// `setWindowText`'s and the gadgets' text setters: `GadgetButtonSetText` / `GadgetCheckBoxSetText` /
    /// `GadgetRadioSetText` / `GadgetStaticTextSetText` (`GGM_SET_LABEL`), `GadgetTextEntrySetText` (`GEM_SET_TEXT`),
    /// else `winSetText`.
    pub fn set_label<H: GuiHost>(&mut self, h: &mut H, w: WinId, text: &[u16]) {
        let style = self.tree.windows[w].inst.style;
        if style & (gws::PUSH_BUTTON | gws::RADIO_BUTTON | gws::CHECK_BOX | gws::STATIC_TEXT) != 0 {
            self.msg_text = text.to_vec();
            self.win_send_system_msg(h, Some(w), ggm::SET_LABEL, 1, 0);
        } else if style & gws::ENTRY_FIELD != 0 {
            crate::client::gadget_text_entry::set_text(self, h, Some(w), text);
        } else {
            self.win_set_text(w, text);
        }
    }

    /// `winSetText`.
    pub fn win_set_text(&mut self, w: WinId, text: &[u16]) {
        self.tree.windows[w].text = (!text.is_empty()).then(|| text.to_vec());
    }

    /// `winGetText`.
    pub fn win_get_text(&self, w: WinId) -> &[u16] {
        self.tree.windows[w].text.as_deref().unwrap_or_default()
    }

    /// `winCreateFromScript(filename, info)` on the file's bytes, then `WindowLayout::load`'s part: the windows the
    /// reader created are run through upstream's creation calls (see the module docs), and the layout is made of the
    /// file's top-level windows with its callbacks. Returns the layout's index and the reader's result.
    pub fn win_create_from_script<W: WndHost, H: GuiHost>(&mut self, script: &mut WindowScript, filename: &[u8], data: &[u8], wnd: &mut W, h: &mut H) -> Result<(Option<usize>, ScriptResult), WndError> {
        let first = self.tree.windows.len();
        let r = script.win_create_from_script(data, &mut self.tree, wnd)?;
        let last = self.tree.windows.len();
        for w in first..last {
            self.adopt(h, w);
        }
        // parseChildWindows: once its children are read, a tab control's panes are its children
        for w in first..last {
            if self.tree.windows[w].inst.style & gws::TAB_CONTROL != 0 && !self.tree.windows[w].children.is_empty() && self.undefined.is_none() {
                crate::client::gadget_tab_control::fixup_sub_pane_list(self, w);
            }
        }
        let Some(info) = r.info.clone() else { return Ok((None, r)) };
        if r.first_window.is_none() {
            return Ok((None, r));
        }
        let l = self.layouts.len();
        self.layouts.push(WindowLayout { filename: filename.to_vec(), ..Default::default() });
        for w in info.windows.iter().flatten() {
            self.layout_add_window(l, *w);
        }
        self.set_layout_callbacks(l, &info);
        Ok((Some(l), r))
    }

    fn set_layout_callbacks(&mut self, l: usize, info: &WindowLayoutInfo) {
        let name = |ok: bool, n: &Vec<u8>| ok.then(|| n.clone());
        self.layouts[l].init = name(info.resolved[0], &info.init_name_string);
        self.layouts[l].update = name(info.resolved[1], &info.update_name_string);
        self.layouts[l].shutdown = name(info.resolved[2], &info.shutdown_name_string);
    }

    /// Upstream's calls for a window the reader created (`createWindow`, `createGadget`, `winCreate`, `gogoGadget*`).
    fn adopt<H: GuiHost>(&mut self, h: &mut H, w: WinId) {
        self.ext.resize(self.tree.windows.len(), WinExt::default());
        let parent = self.tree.windows[w].parent;
        if parent.is_none() {
            self.link_window(w);
        }
        let style = self.tree.windows[w].inst.style;
        let cb = self.tree.windows[w].callbacks.clone();
        let is_gadget = !matches!(self.tree.windows[w].window_type.as_slice(), b"USER" | b"TABPANE");
        // setWindowText comes after the creation: the gadget is created without its text
        let text = self.tree.windows[w].text.take();
        // winCreate's system: the gadget's, or the file's
        let system = if is_gadget { Self::gadget_system(style).unwrap_or(SystemFunc::Default) } else { cb[0].as_deref().map_or(SystemFunc::Default, lexicon_system) };
        self.ext[w].system = system;
        self.win_send_system_msg(h, Some(w), gwm::CREATE, 0, 0);
        if is_gadget {
            let kids = self.tree.windows[w].children.len();
            match gadget_style(style) {
                gws::SCROLL_LISTBOX => {
                    let t = match &self.tree.windows[w].gadget {
                        GadgetData::Listbox(d) => crate::client::gadget_list_box::listbox_template(d),
                        _ => crate::client::gadget_list_box::ListboxRt::default(),
                    };
                    let font = self.tree.windows[w].inst.font.clone();
                    crate::client::gadget_list_box::list_box_after_create(self, h, w, parent, font, t, false);
                    crate::client::gadget_list_box::assign_part_draw_data(self, w);
                }
                gws::VERT_SLIDER | gws::HORZ_SLIDER => {
                    let mut t = match &self.tree.windows[w].gadget {
                        GadgetData::Slider(d) => crate::client::gadget_slider::slider_template(d),
                        _ => crate::client::gadget_slider::SliderRt::default(),
                    };
                    self.tree.windows[w].status |= win_status::TAB_STOP;
                    let status = self.tree.windows[w].status;
                    let font = self.tree.windows[w].inst.font.clone();
                    crate::client::gadget_slider::slider_after_create(self, h, w, parent, status, style, font, &mut t, false);
                    // createGadget: the file's thumb draw data
                    let thumb = self.tree.windows[w].children.first().copied();
                    let part = self.tree.windows[w].parts.iter().find(|p| p.0 == crate::client::window_script::GadgetPart::SliderThumb).map(|p| p.1.clone());
                    if let (Some(t), Some(d)) = (thumb, part) {
                        let i = &mut self.tree.windows[t].inst;
                        i.enabled_draw_data = d.enabled;
                        i.disabled_draw_data = d.disabled;
                        i.hilite_draw_data = d.hilite;
                    }
                }
                gws::ENTRY_FIELD => {
                    let t = match &self.tree.windows[w].gadget {
                        GadgetData::Entry(d) => crate::client::gadget_text_entry::entry_template(d),
                        _ => crate::client::gadget_text_entry::EntryRt::default(),
                    };
                    let font = self.tree.windows[w].inst.font.clone();
                    crate::client::gadget_text_entry::text_entry_after_create(self, h, w, parent, font, t, false);
                    // (the label's text: set once, below, as setWindowText sets it again)
                }
                gws::COMBO_BOX => {
                    let t = match &self.tree.windows[w].gadget {
                        GadgetData::ComboBox(d) => crate::client::gadget_combo_box::combo_template(d),
                        _ => crate::client::gadget_combo_box::combo_template(&Default::default()),
                    };
                    let status = self.tree.windows[w].status;
                    let font = self.tree.windows[w].inst.font.clone();
                    crate::client::gadget_combo_box::combo_box_after_create(self, h, w, parent, status, font, t, false);
                    crate::client::gadget_combo_box::assign_part_draw_data(self, w);
                }
                gws::TAB_CONTROL => {
                    let t = match &self.tree.windows[w].gadget {
                        GadgetData::TabControl(d) => crate::client::gadget_tab_control::tab_template(d),
                        _ => crate::client::gadget_tab_control::TabRt::default(),
                    };
                    let font = self.tree.windows[w].inst.font.clone();
                    let first_new = self.tree.windows.len();
                    crate::client::gadget_tab_control::tab_control_after_create(self, h, w, parent, font, t, false);
                    // parseChildWindows: a tab control with children in the file loses its default panes first
                    if kids > 0 {
                        let panes: Vec<WinId> = (first_new..self.tree.windows.len()).collect();
                        crate::client::gadget_tab_control::destroy_default_panes(self, w, &panes);
                    }
                }
                _ => self.install_gadget(w, parent),
            }
            // the inner windows were created before the file's children: they go after them
            let ch = &mut self.tree.windows[w].children;
            let inner = ch.len() - kids;
            ch.rotate_left(inner);
        }
        // createWindow: the callbacks the file named
        if let Some(s) = &cb[0] {
            self.ext[w].system = lexicon_system(s);
        }
        if let Some(i) = &cb[1] {
            self.ext[w].input = lexicon_input(i);
        }
        if let Some(t) = &cb[2] {
            self.ext[w].tooltip = Some(t.clone());
        }
        if let Some(d) = &cb[3] {
            self.ext[w].draw = lexicon_draw(d);
        }
        // setWindowText: the reader's final text
        if let Some(t) = text {
            self.set_label(h, w, &t);
        }
        if let Some(p) = parent {
            let id = self.tree.windows[w].id;
            self.win_send_input_msg(h, Some(p), gwm::SCRIPT_CREATE, id as u32, 0);
        }
    }

    /// `winDestroy(window)`.
    pub fn win_destroy(&mut self, w: Option<WinId>) -> i32 {
        let Some(w) = w else { return WIN_ERR_INVALID_WINDOW };
        if self.tree.windows[w].status & win_status::DESTROYED != 0 {
            return WIN_ERR_OK;
        }
        self.tree.windows[w].status |= win_status::DESTROYED;
        if self.mouse_captor == Some(w) {
            self.win_release(Some(w));
        }
        if self.keyboard_focus == Some(w) {
            // `winSetFocus(NULL)`: tells the window it loses the focus (its system ignores it: destroyed)
            self.keyboard_focus = None;
        }
        if self.modal.first() == Some(&w) {
            self.win_unset_modal(Some(w));
        }
        if self.curr_mouse_rgn == Some(w) {
            self.curr_mouse_rgn = None;
        }
        if self.grab_window == Some(w) {
            self.grab_window = None;
        }
        for c in self.tree.windows[w].children.clone() {
            self.win_destroy(Some(c));
        }
        if self.tree.windows[w].parent.is_none() {
            self.unlink_window(w);
        } else {
            self.unlink_child_window(w);
        }
        self.destroy_list.insert(0, w);
        if let Some(l) = self.ext[w].layout {
            self.layout_remove_window(l, w);
        }
        WIN_ERR_OK
    }

    /// `processDestroyList`: each window gets `GWM_DESTROY` and is freed.
    pub fn process_destroy_list<H: GuiHost>(&mut self, h: &mut H) {
        let list = std::mem::take(&mut self.destroy_list);
        for w in list {
            if self.mouse_captor == Some(w) {
                self.win_release(Some(w));
            }
            if self.keyboard_focus == Some(w) {
                self.win_set_focus(h, None);
            }
            if self.modal.first() == Some(&w) {
                self.win_unset_modal(Some(w));
            }
            if self.curr_mouse_rgn == Some(w) {
                self.curr_mouse_rgn = None;
            }
            if self.grab_window == Some(w) {
                self.grab_window = None;
            }
            self.win_send_system_msg(h, Some(w), gwm::DESTROY, 0, 0);
            self.ext[w].freed = true;
        }
    }

    /// `update`: the destroy list, then (#77) `TheTransitionHandler->update()`.
    pub fn update<H: GuiHost>(&mut self, h: &mut H) {
        self.process_destroy_list(h);
        crate::client::window_transitions::update(self, h);
    }

    /// `reset`: every window destroyed (`winDestroyAll`), then (#77) `TheTransitionHandler->reset()`.
    pub fn reset<H: GuiHost>(&mut self, h: &mut H) {
        self.win_destroy_all(h);
        self.transitions.reset();
    }

    /// `winDestroyAll`.
    pub fn win_destroy_all<H: GuiHost>(&mut self, h: &mut H) {
        for w in self.window_list.clone() {
            self.win_destroy(Some(w));
        }
        self.process_destroy_list(h);
    }

    // -----------------------------------------------------------------------
    // layouts
    // -----------------------------------------------------------------------

    /// `WindowLayout::addWindow`: at the layout's head, once.
    pub fn layout_add_window(&mut self, l: usize, w: WinId) {
        if self.layouts[l].windows.contains(&w) {
            return;
        }
        self.layouts[l].windows.insert(0, w);
        self.ext[w].layout = Some(l);
    }

    /// `WindowLayout::removeWindow`.
    pub fn layout_remove_window(&mut self, l: usize, w: WinId) {
        if let Some(i) = self.layouts[l].windows.iter().position(|&x| x == w) {
            self.layouts[l].windows.remove(i);
            self.ext[w].layout = None;
        }
    }

    /// `WindowLayout::hide(hide)`.
    pub fn layout_hide(&mut self, l: usize, hide: bool) {
        for w in self.layouts[l].windows.clone() {
            self.win_hide(w, hide);
        }
        self.layouts[l].hidden = hide;
    }

    /// `WindowLayout::destroyWindows`.
    pub fn layout_destroy_windows(&mut self, l: usize) {
        while let Some(&w) = self.layouts[l].windows.first() {
            self.layout_remove_window(l, w);
            self.win_destroy(Some(w));
        }
    }

    /// `WindowLayout::bringForward`: from the tail, each to the top.
    pub fn layout_bring_forward(&mut self, l: usize) {
        let mut ws = self.layouts[l].windows.clone();
        ws.reverse();
        for w in ws {
            self.win_bring_to_top(w);
        }
    }

    /// `runInit`, `runUpdate`, `runShutdown` (0, 1, 2): the layout's callback, if the lexicon had it.
    pub fn layout_run<H: GuiHost>(&mut self, h: &mut H, l: usize, which: usize) {
        let f = match which {
            0 => self.layouts[l].init.clone(),
            1 => self.layouts[l].update.clone(),
            _ => self.layouts[l].shutdown.clone(),
        };
        if let Some(f) = f {
            h.layout(self, &f, l);
        }
    }

    // -----------------------------------------------------------------------
    // drawing
    // -----------------------------------------------------------------------

    /// `winRepaint`: the window list from its tail, `BELOW`, then neither, then `ABOVE` windows.
    pub fn win_repaint<H: GuiHost>(&mut self, h: &mut H) -> Vec<DrawItem> {
        let mut out = Vec::new();
        let tail_first: Vec<WinId> = self.window_list.iter().rev().copied().collect();
        for pass in 0..3 {
            for &w in &tail_first {
                let s = self.tree.windows[w].status;
                let draw = match pass {
                    0 => s & win_status::BELOW != 0,
                    1 => s & (win_status::ABOVE | win_status::BELOW) == 0,
                    _ => s & win_status::ABOVE != 0,
                };
                if draw {
                    self.draw_window(h, w, &mut out);
                }
            }
        }
        // (#77) `TheTransitionHandler->draw()`
        crate::client::window_transitions::draw(self, h, &mut out);
        out
    }

    /// `drawWindow(window)`.
    pub fn draw_window<H: GuiHost>(&mut self, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
        let s = self.tree.windows[w].status;
        if s & win_status::HIDDEN != 0 {
            return;
        }
        if s & win_status::SEE_THRU == 0 {
            crate::client::window_draw::draw(self, h, w, out);
        }
        let listbox = self.tree.windows[w].inst.style & gws::SCROLL_LISTBOX != 0;
        let border = s & win_status::BORDER != 0 && s & win_status::SEE_THRU == 0;
        if listbox && border {
            crate::client::window_draw::win_draw_border(self, h, w, out);
        }
        let mut kids = self.tree.windows[w].children.clone();
        kids.reverse();
        for c in kids {
            self.draw_window(h, c, out);
        }
        if !listbox && border {
            crate::client::window_draw::win_draw_border(self, h, w, out);
        }
    }
}

/// `winFindFont("Times New Roman", 14, FALSE)`, the default when no language data gives one.
pub(crate) fn default_font() -> GameFont {
    GameFont { name: b"Times New Roman".to_vec(), point_size: 14, bold: false }
}

/// `GameWindow::normalizeWindowRegion`: the lower corner in `lo`.
fn normalize_window_region(w: &mut GameWindow) {
    let r = &mut w.region;
    if r.0 > r.2 {
        std::mem::swap(&mut r.0, &mut r.2);
    }
    if r.1 > r.3 {
        std::mem::swap(&mut r.1, &mut r.3);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::client::gadget;
    use crate::client::window_script::tests::Host as WndTestHost;

    /// A host that records what its callbacks get, and handles what `handle` lists.
    #[derive(Default)]
    pub(crate) struct Rec {
        pub sys: Vec<(Vec<u8>, WinId, u32, u32)>,
        pub inp: Vec<(Vec<u8>, WinId, u32, u32)>,
        pub audio: Vec<Vec<u8>>,
        pub layouts: Vec<Vec<u8>>,
        /// Input messages the host's input callbacks handle.
        pub handle: Vec<u32>,
        /// Whether the host's system callbacks want the focus.
        pub want_focus: bool,
    }

    impl GuiHost for Rec {
        fn system(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: u32, _d2: u32) -> MsgHandled {
            self.sys.push((func.to_vec(), win, msg, d1));
            if msg == gwm::INPUT_FOCUS && self.want_focus {
                wm.focus_reply = true;
            }
            MsgHandled::Handled
        }
        fn input(&mut self, _wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: u32, _d2: u32) -> MsgHandled {
            self.inp.push((func.to_vec(), win, msg, d1));
            if self.handle.contains(&msg) { MsgHandled::Handled } else { MsgHandled::Ignored }
        }
        fn layout(&mut self, _wm: &mut GameWindowManager, func: &[u8], _l: usize) {
            self.layouts.push(func.to_vec());
        }
        fn add_audio_event(&mut self, name: &[u8]) {
            self.audio.push(name.to_vec());
        }
        fn image_size(&self, image: &[u8]) -> Option<(i32, i32)> {
            Some(if image.ends_with(b"Mid") { (7, 10) } else { (5, 10) })
        }
        fn has_image(&self, _image: &[u8]) -> bool {
            true
        }
        fn printable_key(&self, key: u8, state: i32) -> u16 {
            if state == 1 { u16::from(key) } else { 0 }
        }
    }

    /// A layout at 800x600 (scale 1): `Parent` (0..400, 0..300, `TestSystem` / `TestInput`, layout callbacks
    /// `TestInit`), with a mouse-tracking push button `B1` at (10,10)-(60,40), a push button `B2` at (70,10)-(120,40),
    /// a mouse-tracking check box `C` at (10,50)-(110,70), a check box `C2` without tracking at (10,80)-(110,100), a static
    /// text `T` at (10,110)-(110,130), and three radio buttons `R1..R3` of group 1 at y 140, 170, 200.
    pub(crate) const LAYOUT: &[u8] = b"FILE_VERSION = 2;\n\
STARTLAYOUTBLOCK\n  LAYOUTINIT = TestInit;\n  LAYOUTUPDATE = [None];\n  LAYOUTSHUTDOWN = [None];\nENDLAYOUTBLOCK\n\
WINDOW\n  WINDOWTYPE = USER;\n  SCREENRECT = UPPERLEFT: 0 0, BOTTOMRIGHT: 400 300, CREATIONRESOLUTION: 800 600;\n\
  NAME = \"T.wnd:Parent\";\n  STATUS = ENABLED;\n  STYLE = USER;\n  SYSTEMCALLBACK = \"TestSystem\";\n  INPUTCALLBACK = \"TestInput\";\n\
  CHILD\n\
  WINDOW\n    WINDOWTYPE = PUSHBUTTON;\n    SCREENRECT = UPPERLEFT: 10 10, BOTTOMRIGHT: 60 40, CREATIONRESOLUTION: 800 600;\n\
    NAME = \"T.wnd:B1\";\n    STATUS = ENABLED;\n    STYLE = PUSHBUTTON+MOUSETRACK;\n    TEXT = \"L:B1\";\n    TOOLTIPTEXT = \"TIP:B1\";\n    TOOLTIPDELAY = 100;\n\
  END\n\
  WINDOW\n    WINDOWTYPE = PUSHBUTTON;\n    SCREENRECT = UPPERLEFT: 70 10, BOTTOMRIGHT: 120 40, CREATIONRESOLUTION: 800 600;\n\
    NAME = \"T.wnd:B2\";\n    STATUS = ENABLED;\n    STYLE = PUSHBUTTON;\n\
  END\n\
  WINDOW\n    WINDOWTYPE = CHECKBOX;\n    SCREENRECT = UPPERLEFT: 10 50, BOTTOMRIGHT: 110 70, CREATIONRESOLUTION: 800 600;\n\
    NAME = \"T.wnd:C\";\n    STATUS = ENABLED;\n    STYLE = CHECKBOX+MOUSETRACK;\n\
  END\n\
  WINDOW\n    WINDOWTYPE = CHECKBOX;\n    SCREENRECT = UPPERLEFT: 10 80, BOTTOMRIGHT: 110 100, CREATIONRESOLUTION: 800 600;\n\
    NAME = \"T.wnd:C2\";\n    STATUS = ENABLED;\n    STYLE = CHECKBOX;\n\
  END\n\
  WINDOW\n    WINDOWTYPE = STATICTEXT;\n    SCREENRECT = UPPERLEFT: 10 110, BOTTOMRIGHT: 110 130, CREATIONRESOLUTION: 800 600;\n\
    NAME = \"T.wnd:T\";\n    STATUS = ENABLED;\n    STYLE = STATICTEXT;\n    STATICTEXTDATA = CENTERED: 1;\n    TEXT = \"L:T\";\n\
  END\n\
  WINDOW\n    WINDOWTYPE = RADIOBUTTON;\n    SCREENRECT = UPPERLEFT: 10 140, BOTTOMRIGHT: 110 160, CREATIONRESOLUTION: 800 600;\n\
    NAME = \"T.wnd:R1\";\n    STATUS = ENABLED;\n    STYLE = RADIOBUTTON+MOUSETRACK;\n    RADIOBUTTONDATA = GROUP: 1;\n\
  END\n\
  WINDOW\n    WINDOWTYPE = RADIOBUTTON;\n    SCREENRECT = UPPERLEFT: 10 170, BOTTOMRIGHT: 110 190, CREATIONRESOLUTION: 800 600;\n\
    NAME = \"T.wnd:R2\";\n    STATUS = ENABLED;\n    STYLE = RADIOBUTTON+MOUSETRACK;\n    RADIOBUTTONDATA = GROUP: 1;\n\
  END\n\
  WINDOW\n    WINDOWTYPE = RADIOBUTTON;\n    SCREENRECT = UPPERLEFT: 10 200, BOTTOMRIGHT: 110 220, CREATIONRESOLUTION: 800 600;\n\
    NAME = \"T.wnd:R3\";\n    STATUS = ENABLED;\n    STYLE = RADIOBUTTON+MOUSETRACK;\n    RADIOBUTTONDATA = GROUP: 1;\n\
  END\n\
  ENDALLCHILDREN\n\
END\n";

    /// The windows of [`LAYOUT`], by short name.
    pub(crate) struct Ids {
        pub p: WinId,
        pub b1: WinId,
        pub b2: WinId,
        pub c: WinId,
        pub c2: WinId,
        pub t: WinId,
        pub r: [WinId; 3],
    }

    pub(crate) struct WndHost800(pub(crate) WndTestHost);
    impl WndHost for WndHost800 {
        fn name_to_key(&mut self, name: &[u8]) -> u32 {
            self.0.name_to_key(name)
        }
        fn find_image(&self, name: &[u8]) -> Option<Vec<u8>> {
            self.0.find_image(name)
        }
        fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
            self.0.fetch_text(label)
        }
        fn display_size(&self) -> (i32, i32) {
            (800, 600)
        }
    }

    pub(crate) fn load(h: &mut Rec) -> (GameWindowManager, Ids, usize) {
        let mut wm = GameWindowManager::new((800, 600));
        let mut wnd = WndHost800(WndTestHost::default());
        let (l, _) = wm.win_create_from_script(&mut WindowScript::new(), b"T.wnd", LAYOUT, &mut wnd, h).expect("reads");
        let f = |n: &str| wm.tree.find_by_name(format!("T.wnd:{n}").as_bytes()).expect(n);
        let ids = Ids { p: f("Parent"), b1: f("B1"), b2: f("B2"), c: f("C"), c2: f("C2"), t: f("T"), r: [f("R1"), f("R2"), f("R3")] };
        (wm, ids, l.expect("a layout"))
    }

    fn s(t: &str) -> Vec<u16> {
        t.encode_utf16().collect()
    }

    /// #34: creation. Predicted from GameWindowManager.cpp / GameWindowManagerScript.cpp before the run:
    /// - the parent's system (`TestSystem`, the host's) gets `GWM_CREATE` first; then each child, created in file order,
    ///   sends the parent's input (`TestInput`) `GWM_SCRIPT_CREATE` with its id: eight, in the order B1, B2, C, C2, T,
    ///   R1, R2, R3; the gadgets' own `GWM_CREATE`s go to their gadget systems, not the host;
    /// - the window list is [Parent]; the parent's children newest first, [R3, R2, R1, T, C2, C, B2, B1];
    /// - each gadget's owner is the parent; B1's text is `T:L:B1` (fetched), the static text's text is in its
    ///   `TextData` (`T:L:T`) and not in the window; the radio buttons' user data is group 1, screen the key of `T.wnd`;
    /// - the layout holds [Parent] and its init callback `TestInit` runs on `runInit`; update is `[None]`, not run;
    /// - the push buttons draw with `W3DGadgetPushButtonDraw` (no `IMAGE` status), the parent with the W3D default.
    #[test]
    fn script_windows_are_created_as_upstream_creates_them() {
        let mut h = Rec::default();
        let (mut wm, ids, l) = load(&mut h);
        assert_eq!(h.sys, vec![(b"TestSystem".to_vec(), ids.p, gwm::CREATE, 0)]);
        let order = [ids.b1, ids.b2, ids.c, ids.c2, ids.t, ids.r[0], ids.r[1], ids.r[2]];
        let script: Vec<_> = order.iter().map(|&w| (b"TestInput".to_vec(), ids.p, gwm::SCRIPT_CREATE, wm.win(w).id as u32)).collect();
        assert_eq!(h.inp, script);
        assert_eq!(wm.window_list, vec![ids.p]);
        let mut rev = order.to_vec();
        rev.reverse();
        assert_eq!(wm.win(ids.p).children, rev);
        for &w in &order {
            assert_eq!(wm.win_get_owner(w), Some(ids.p));
        }
        assert_eq!(wm.win(ids.b1).text, Some(s("T:L:B1")));
        assert_eq!(wm.win(ids.t).text, None);
        assert_eq!(gadget::static_text_get_text(&wm, ids.t), s("T:L:T"));
        let UserData::Radio(r) = wm.ext[ids.r[1]].user else { panic!("radio data") };
        assert_eq!(r.group, 1);
        assert_ne!(r.screen, 0);
        assert_eq!(wm.layouts[l].windows, vec![ids.p]);
        assert_eq!(wm.ext[ids.b1].draw, DrawFunc::PushButton);
        assert_eq!(wm.ext[ids.p].draw, DrawFunc::W3DDefault);
        wm.layout_run(&mut h, l, 0);
        wm.layout_run(&mut h, l, 1);
        assert_eq!(h.layouts, vec![b"TestInit".to_vec()]);
    }

    /// #34: the mouse through a push button. Predicted from `winProcessMouseEvent` and `GadgetPushButtonInput`:
    /// - moving onto B1 (mouse tracking) hilites it and sends the parent `GBM_MOUSE_ENTERING` with B1;
    /// - the left press selects B1, plays `GUIClick`, makes B1 the grab window, and is used;
    /// - the release over it sends the parent `GBM_SELECTED` with B1, clears the selection and the grab;
    /// - a press on B1 released over B2: no `GBM_SELECTED` (the release is not over the grab window), B1 stays selected
    ///   until the next move (while grabbed no leave is sent), whose `GWM_MOUSE_LEAVING` clears it;
    /// - a press on B2 (no mouse tracking): no hilite, no `GBM_MOUSE_ENTERING`, but the click still selects.
    #[test]
    fn a_push_button_is_pressed_through_the_manager() {
        let mut h = Rec::default();
        let (mut wm, ids, _) = load(&mut h);
        h.sys.clear();
        let b1d = win_data(Some(ids.b1));
        wm.win_process_mouse_event(&mut h, gwm::MOUSE_POS, (20, 20), (0, 0));
        assert_eq!(wm.curr_mouse_rgn, Some(ids.b1));
        assert!(wm.win(ids.b1).inst.state & WIN_STATE_HILITED != 0);
        assert_eq!(h.sys.last(), Some(&(b"TestSystem".to_vec(), ids.p, ggm::GBM_MOUSE_ENTERING, b1d)));
        assert_eq!(wm.win_process_mouse_event(&mut h, gwm::LEFT_DOWN, (20, 20), (0, 0)), WinInputReturnCode::Used);
        assert_eq!((wm.grab_window, h.audio.clone()), (Some(ids.b1), vec![b"GUIClick".to_vec()]));
        assert!(wm.win(ids.b1).inst.state & WIN_STATE_SELECTED != 0);
        wm.win_process_mouse_event(&mut h, gwm::LEFT_UP, (21, 21), (0, 0));
        assert_eq!(h.sys.last(), Some(&(b"TestSystem".to_vec(), ids.p, ggm::GBM_SELECTED, b1d)));
        assert_eq!((wm.grab_window, wm.win(ids.b1).inst.state & WIN_STATE_SELECTED), (None, 0));
        // pressed on B1, released over B2
        let n = h.sys.len();
        wm.win_process_mouse_event(&mut h, gwm::LEFT_DOWN, (20, 20), (0, 0));
        wm.win_process_mouse_event(&mut h, gwm::MOUSE_POS, (80, 20), (0, 0));
        wm.win_process_mouse_event(&mut h, gwm::LEFT_UP, (80, 20), (0, 0));
        assert!(h.sys[n..].iter().all(|m| m.2 != ggm::GBM_SELECTED), "{:?}", &h.sys[n..]);
        assert!(wm.win(ids.b1).inst.state & WIN_STATE_SELECTED != 0);
        wm.win_process_mouse_event(&mut h, gwm::MOUSE_POS, (81, 20), (0, 0));
        assert_eq!(wm.win(ids.b1).inst.state & (WIN_STATE_SELECTED | WIN_STATE_HILITED), 0);
        assert_eq!(wm.curr_mouse_rgn, Some(ids.b2));
        // B2 does not track the mouse
        assert!(wm.win(ids.b2).inst.state & WIN_STATE_HILITED == 0);
        assert!(!h.sys.iter().any(|m| m.2 == ggm::GBM_MOUSE_ENTERING && m.3 == win_data(Some(ids.b2))));
        wm.win_process_mouse_event(&mut h, gwm::LEFT_DOWN, (80, 20), (0, 0));
        wm.win_process_mouse_event(&mut h, gwm::LEFT_UP, (80, 20), (0, 0));
        assert_eq!(h.sys.last(), Some(&(b"TestSystem".to_vec(), ids.p, ggm::GBM_SELECTED, win_data(Some(ids.b2)))));
    }

    /// #34: hidden, disabled, captured and modal windows. Predicted:
    /// - B1 disabled: a press there finds the parent (`winPointInChild` skips it), whose host input ignores it: not
    ///   used, no grab; ~~the parent's input gets only the press~~ **missed on the first run:** it also gets
    ///   `GWM_MOUSE_ENTERING` after it, since the parent is now the window under the pointer (no window was before):
    ///   the prediction forgot the enter/leave step at the end of `winProcessMouseEvent`; no sound plays (only a grab
    ///   window's release plays `GUIClickDisabled`);
    /// - B1 hidden: the same;
    /// - the parent captured: a press on B2 goes to B2 (the innermost child) and is used; a press outside every window
    ///   still goes to the captor's tree (the captor itself), to the parent's input;
    /// - a modal top-level window `M` (with nothing in it): a press over B2 goes to `M` (`winPointInChild` of the modal
    ///   window returns it wherever the point is), its default input ignores it: not used; B2 sees nothing.
    #[test]
    fn hidden_disabled_captured_and_modal_windows_route_as_upstream() {
        let mut h = Rec::default();
        let (mut wm, ids, _) = load(&mut h);
        wm.win_enable(ids.b1, false);
        h.inp.clear();
        assert_eq!(wm.win_process_mouse_event(&mut h, gwm::LEFT_DOWN, (20, 20), (0, 0)), WinInputReturnCode::NotUsed);
        assert_eq!(h.inp, vec![(b"TestInput".to_vec(), ids.p, gwm::LEFT_DOWN, short_to_long(20, 20)), (b"TestInput".to_vec(), ids.p, gwm::MOUSE_ENTERING, short_to_long(20, 20))]);
        assert_eq!(wm.grab_window, None);
        wm.win_enable(ids.b1, true);
        wm.win_hide(ids.b1, true);
        assert_eq!(wm.win_process_mouse_event(&mut h, gwm::LEFT_DOWN, (20, 20), (0, 0)), WinInputReturnCode::NotUsed);
        assert!(wm.win(ids.b1).inst.state & WIN_STATE_SELECTED == 0);
        wm.win_hide(ids.b1, false);
        assert!(h.audio.is_empty());
        // capture
        assert_eq!(wm.win_capture(Some(ids.p)), WIN_ERR_OK);
        assert_eq!(wm.win_capture(Some(ids.b1)), WIN_ERR_MOUSE_CAPTURED);
        assert_eq!(wm.win_process_mouse_event(&mut h, gwm::LEFT_DOWN, (80, 20), (0, 0)), WinInputReturnCode::Used);
        assert!(wm.win(ids.b2).inst.state & WIN_STATE_SELECTED != 0);
        h.inp.clear();
        wm.win_process_mouse_event(&mut h, gwm::RIGHT_DOWN, (700, 500), (0, 0));
        assert_eq!(h.inp.first().map(|m| (m.1, m.2)), Some((ids.p, gwm::RIGHT_DOWN)));
        wm.win_release(Some(ids.p));
        // modal
        let m = wm.win_create(&mut h, None, win_status::ENABLED, 500, 400, 100, 100, SystemFunc::Default, None);
        assert_eq!(wm.window_list, vec![m, ids.p]);
        assert_eq!(wm.win_set_modal(Some(ids.b1)), WIN_ERR_INVALID_PARAMETER);
        assert_eq!(wm.win_set_modal(Some(m)), WIN_ERR_OK);
        wm.win_mut(ids.b2).inst.state = 0;
        assert_eq!(wm.win_process_mouse_event(&mut h, gwm::LEFT_DOWN, (80, 20), (0, 0)), WinInputReturnCode::NotUsed);
        assert_eq!(wm.win(ids.b2).inst.state, 0);
        assert_eq!(wm.curr_mouse_rgn, Some(m));
        // a window created while M is modal goes behind it
        let n = wm.win_create(&mut h, None, win_status::ENABLED, 0, 0, 10, 10, SystemFunc::Default, None);
        assert_eq!(wm.window_list, vec![m, n, ids.p]);
        assert_eq!(wm.win_unset_modal(Some(ids.p)), WIN_ERR_GENERAL_FAILURE);
        assert_eq!(wm.win_unset_modal(Some(m)), WIN_ERR_OK);
    }

    /// #34: the keyboard and the focus. Predicted from `winSetFocus`, `winProcessKey` and the push button:
    /// - focus on B1: B1 hilited, the parent gets `GGM_FOCUS_CHANGE` (1, B1's id); B1 keeps the focus;
    /// - enter down selects B1 (no message), enter up sends `GBM_SELECTED` and clears it; both used;
    /// - a key B1 ignores (`ESC`) goes up to the parent's input; the host ignores it: not used;
    /// - focus on the static text: its system ignores `GWM_INPUT_FOCUS`, the parent (host, not wanting it) is asked
    ///   next: nobody has the focus; with the host wanting it, the **static text** keeps the focus (the quirk);
    /// - focus on B2 then B1: B2 is told it lost it (unhilited, `GGM_FOCUS_CHANGE` 0).
    #[test]
    fn the_focus_and_the_keyboard_route_as_upstream() {
        let mut h = Rec::default();
        let (mut wm, ids, _) = load(&mut h);
        h.sys.clear();
        wm.win_set_focus(&mut h, Some(ids.b1));
        assert_eq!(wm.keyboard_focus, Some(ids.b1));
        assert!(wm.win(ids.b1).inst.state & WIN_STATE_HILITED != 0);
        assert_eq!(h.sys, vec![(b"TestSystem".to_vec(), ids.p, ggm::FOCUS_CHANGE, 1)]);
        assert_eq!(wm.win_process_key(&mut h, key::ENTER, key::STATE_DOWN), WinInputReturnCode::Used);
        assert!(wm.win(ids.b1).inst.state & WIN_STATE_SELECTED != 0);
        assert_eq!(wm.win_process_key(&mut h, key::ENTER, key::STATE_UP), WinInputReturnCode::Used);
        assert_eq!(h.sys.last().map(|m| (m.2, m.3)), Some((ggm::GBM_SELECTED, win_data(Some(ids.b1)))));
        h.inp.clear();
        assert_eq!(wm.win_process_key(&mut h, key::ESC, key::STATE_DOWN), WinInputReturnCode::NotUsed);
        assert_eq!(h.inp, vec![(b"TestInput".to_vec(), ids.p, gwm::CHAR, u32::from(key::ESC))]);
        wm.win_set_focus(&mut h, Some(ids.t));
        assert_eq!(wm.keyboard_focus, None);
        assert!(wm.win(ids.b1).inst.state & WIN_STATE_HILITED == 0, "B1 told it lost the focus");
        h.want_focus = true;
        wm.win_set_focus(&mut h, Some(ids.t));
        assert_eq!(wm.keyboard_focus, Some(ids.t));
        h.want_focus = false;
        wm.win_set_focus(&mut h, Some(ids.b2));
        h.sys.clear();
        wm.win_set_focus(&mut h, Some(ids.b1));
        assert_eq!(h.sys[0], (b"TestSystem".to_vec(), ids.p, ggm::FOCUS_CHANGE, 0));
        assert!(wm.win(ids.b2).inst.state & WIN_STATE_HILITED == 0);
        // the tab list: B1 -> B2, wrapping; nothing while a modal window is up
        wm.register_tab_list(vec![ids.b1, ids.b2]);
        wm.win_process_key(&mut h, key::TAB, key::STATE_DOWN);
        assert_eq!(wm.keyboard_focus, Some(ids.b2));
        wm.win_process_key(&mut h, key::TAB, key::STATE_DOWN);
        assert_eq!(wm.keyboard_focus, Some(ids.b1));
    }

    /// #34: the check box and radio buttons. Predicted from GadgetCheckBox.cpp and GadgetRadioButton.cpp:
    /// - C (mouse tracking): a click toggles it on, `GBM_SELECTED`; again, off;
    /// - C2 (no tracking): never hilited, so its release is ignored: the click does nothing (the quirk);
    /// - C's right release unchecks a checked box and sends `GBM_SELECTED_RIGHT`;
    /// - R2 clicked: selected, `GBM_SELECTED`; R3 clicked: R3 selected, R2 not; clicking R3 again: no message;
    /// - `GadgetRadioSetSelection(R1, TRUE)`: R1 selected, R3 not, and **no** `GBM_SELECTED` (the pointer quirk);
    /// - a radio button in another group stays selected.
    #[test]
    fn check_boxes_and_radio_buttons_behave_as_upstream() {
        let mut h = Rec::default();
        let (mut wm, ids, _) = load(&mut h);
        let click = |wm: &mut GameWindowManager, h: &mut Rec, p: (i32, i32)| {
            wm.win_process_mouse_event(h, gwm::MOUSE_POS, p, (0, 0));
            wm.win_process_mouse_event(h, gwm::LEFT_DOWN, p, (0, 0));
            wm.win_process_mouse_event(h, gwm::LEFT_UP, p, (0, 0));
        };
        click(&mut wm, &mut h, (20, 60));
        assert!(gadget::check_box_is_checked(&wm, ids.c));
        assert_eq!(h.sys.last().map(|m| (m.2, m.3)), Some((ggm::GBM_SELECTED, win_data(Some(ids.c)))));
        click(&mut wm, &mut h, (21, 60));
        assert!(!gadget::check_box_is_checked(&wm, ids.c));
        let n = h.sys.len();
        click(&mut wm, &mut h, (20, 90));
        assert!(!gadget::check_box_is_checked(&wm, ids.c2));
        assert!(h.sys[n..].iter().all(|m| m.3 != win_data(Some(ids.c2)) || m.2 == ggm::GBM_MOUSE_LEAVING), "{:?}", &h.sys[n..]);
        click(&mut wm, &mut h, (20, 60));
        wm.win_process_mouse_event(&mut h, gwm::RIGHT_DOWN, (20, 60), (0, 0));
        wm.win_process_mouse_event(&mut h, gwm::RIGHT_UP, (20, 60), (0, 0));
        assert!(!gadget::check_box_is_checked(&wm, ids.c));
        assert_eq!(h.sys.last().map(|m| m.2), Some(ggm::GBM_SELECTED_RIGHT));
        // radio buttons
        let other = wm.gogo_gadget(&mut h, Some(ids.p), win_status::ENABLED, 200, 10, 50, 20, WinInstanceData { style: gws::RADIO_BUTTON, ..WinInstanceData::new() }, GadgetData::Radio(RadioButtonData { screen: 0, group: 2 })).expect("radio");
        let UserData::Radio(r1) = wm.ext[ids.r[0]].user else { panic!() };
        gadget::radio_set_group(&mut wm, other, 1, r1.screen + 1);
        wm.win_mut(other).inst.state |= WIN_STATE_SELECTED;
        click(&mut wm, &mut h, (20, 180));
        assert_eq!(h.sys.last().map(|m| (m.2, m.3)), Some((ggm::GBM_SELECTED, win_data(Some(ids.r[1])))));
        click(&mut wm, &mut h, (20, 210));
        let sel = |wm: &GameWindowManager, w: WinId| wm.win(w).inst.state & WIN_STATE_SELECTED != 0;
        assert_eq!([sel(&wm, ids.r[0]), sel(&wm, ids.r[1]), sel(&wm, ids.r[2])], [false, false, true]);
        let n = h.sys.len();
        click(&mut wm, &mut h, (21, 210));
        assert!(h.sys[n..].iter().all(|m| m.2 != ggm::GBM_SELECTED));
        gadget::radio_set_selection(&mut wm, &mut h, ids.r[0], true);
        assert_eq!([sel(&wm, ids.r[0]), sel(&wm, ids.r[1]), sel(&wm, ids.r[2])], [true, false, false]);
        assert!(h.sys[n..].iter().all(|m| m.2 != ggm::GBM_SELECTED));
        assert!(sel(&wm, other), "another screen's group is left alone");
    }

    /// #72: a window destroyed below the top of the modal stack stays in it (`winDestroy` unsets only the top); once
    /// it is freed and the top is unset, a mouse event searches it, and upstream reads freed memory. Predicted: the
    /// stack is `[b, a]` after `a` is freed; the event is recorded as undefined and not used.
    #[test]
    fn a_freed_modal_window_is_undefined_upstream() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let a = wm.win_create(&mut h, None, win_status::ENABLED, 0, 0, 100, 100, SystemFunc::Default, None);
        let b = wm.win_create(&mut h, None, win_status::ENABLED, 200, 0, 100, 100, SystemFunc::Default, None);
        wm.win_set_modal(Some(a));
        wm.win_set_modal(Some(b));
        wm.win_destroy(Some(a));
        wm.update(&mut h);
        assert_eq!(wm.modal, vec![b, a]);
        wm.win_unset_modal(Some(b));
        assert_eq!(wm.win_process_mouse_event(&mut h, gwm::MOUSE_POS, (10, 10), (0, 0)), WinInputReturnCode::NotUsed);
        assert!(wm.undefined.is_some());
    }

    /// #34: destruction, the lone window and the tooltip timer. Predicted:
    /// - a lone window `L` (a host system): a click that B1 takes sends `L` `GGM_CLOSE` and clears it; a click nobody
    ///   takes, on nothing, does too (a release);
    /// - B1's tooltip (`T:TIP:B1`, delay 100): set into the mouse at the move; shown at 100 ms still, not at 99; a move
    ///   restarts the wait; a move onto the parent (no tooltip) clears it;
    /// - destroying the parent: it and its eight children are destroyed and on the destroy list (the parent first,
    ///   then its children as they were destroyed, newest at the head), the window list empty, the layout empty;
    ///   messages to them are ignored; `update` sends the parent's host `GWM_DESTROY` and frees all nine.
    #[test]
    fn destruction_lone_windows_and_tooltips_follow_upstream() {
        let mut h = Rec::default();
        let (mut wm, ids, l) = load(&mut h);
        let lone = wm.win_create(&mut h, None, win_status::ENABLED | win_status::HIDDEN, 0, 0, 10, 10, SystemFunc::Host(b"LoneSystem".to_vec()), None);
        wm.win_set_lone_window(&mut h, Some(lone));
        h.sys.clear();
        wm.win_process_mouse_event(&mut h, gwm::LEFT_DOWN, (20, 20), (0, 0));
        assert_eq!(wm.lone_window, None);
        assert!(h.sys.contains(&(b"LoneSystem".to_vec(), lone, ggm::CLOSE, 0)));
        wm.win_process_mouse_event(&mut h, gwm::LEFT_UP, (20, 20), (0, 0));
        // tooltips
        wm.win_process_mouse_event(&mut h, gwm::MOUSE_POS, (25, 25), (0, 0));
        wm.mouse.create_stream_messages(1000, true);
        assert_eq!(wm.mouse.delay, 100);
        wm.mouse.create_stream_messages(1099, false);
        assert_eq!(wm.mouse.shown(), None);
        wm.mouse.create_stream_messages(1100, false);
        assert_eq!(wm.mouse.shown(), Some(s("T:TIP:B1").as_slice()));
        wm.mouse.create_stream_messages(1150, true);
        wm.mouse.create_stream_messages(1200, false);
        assert_eq!(wm.mouse.shown(), None);
        wm.win_process_mouse_event(&mut h, gwm::MOUSE_POS, (300, 250), (0, 0));
        wm.mouse.create_stream_messages(5000, false);
        assert_eq!(wm.mouse.shown(), None);
        // destruction
        wm.win_destroy(Some(ids.p));
        assert!(!wm.window_list.contains(&ids.p));
        assert_eq!(wm.destroy_list.len(), 9);
        assert_eq!(wm.destroy_list[0], ids.p);
        assert!(wm.layouts[l].windows.is_empty());
        h.sys.clear();
        wm.win_send_system_msg(&mut h, Some(ids.p), gwm::USER, 0, 0);
        assert!(h.sys.is_empty());
        wm.update(&mut h);
        assert_eq!(h.sys, vec![(b"TestSystem".to_vec(), ids.p, gwm::DESTROY, 0)]);
        assert!(wm.destroy_list.is_empty());
        assert!([ids.p, ids.b1, ids.t, ids.r[2]].iter().all(|&w| wm.ext[w].freed));
        assert_eq!(wm.window_list, vec![lone]);
    }
}
