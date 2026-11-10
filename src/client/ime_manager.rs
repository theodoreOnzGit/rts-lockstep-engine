// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/IMEManager.cpp (IMEManager::IMEManager's m_window, m_result,
//     m_disabled, m_composing; attach, detatch, isAttachedTo, getWindow, result, isComposing; serviceIMEMessage's
//     WM_CHAR case)
//   GeneralsMD/Code/Main/WinMain.cpp (WndProc's call: the manager sees each window message first)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The IME manager (GitHub #74): how a typed character reaches the text entry that has the focus.
//!
//! # As upstream reads it
//!
//! `TheIMEManager` (made by `GameClient::init`) is **attached** to at most one window, `m_window`: the text entry that
//! took the keyboard focus (`GadgetTextEntrySystem`'s `GWM_INPUT_FOCUS` calls `attach(window)`, and `attach(NULL)` when
//! it loses the focus while attached). `attach(window)` does nothing for the window already attached; else it
//! `detatch`es (forgets the window) and, unless the IME is disabled, associates the input context with the
//! application's window, then remembers the new window. The shell `detatch`es it in `pop`, `popImmediate`, `hide`,
//! `hideShell`, `reset`, `doPush` and `doPop` (not in `push`: that call is commented out).
//!
//! `WndProc` gives every window message to `serviceIMEMessage` first; if it returns `TRUE` the message is eaten and
//! `result()` is what the window procedure returns. Its `WM_CHAR` case: the UTF-16 unit in the low 16 bits of `wParam`;
//! if a window is attached and the unit is 32 or more, or `VK_RETURN` (13), the window gets
//! `winSendInputMsg(window, GWM_IME_CHAR, wParam & 0xFFFF, lParam)`, `m_result` is 0 and the message is eaten;
//! otherwise it is not (the application's own handling, and `DefWindowProc`, get it). The text entry appends the
//! character, or ends the edit on return ([`crate::client::gadget_text_entry`]).
//!
//! The keys themselves (backspace, the arrows, tab, delete) are not characters: they come from DirectInput as
//! `GWM_CHAR` through `winProcessKey` (the host's to send, see [`crate::client::window_manager::key`]).
//!
//! # Upstream quirks that are kept
//!
//! - Nothing detaches the manager from a window that is **destroyed** while it has the focus: `winDestroy` takes the
//!   focus away, but the destroyed window's system callback no longer runs (`winSendSystemMsg` ignores a destroyed
//!   window), so `attach(NULL)` is never called. Until the shell detaches it, a character is sent to the destroyed
//!   window, which ignores it; once `processDestroyList` has freed it, upstream sends it to freed memory: recorded in
//!   [`GameWindowManager::undefined`] and the message is not eaten.
//! - A control character other than return (backspace's 8, tab's 9, escape's 27) is not eaten and does nothing here.
//! - `WM_IME_CHAR` (not translated, see below) takes characters **above** 32, not 32 and above: a space composed by an
//!   IME would not be sent. `WM_CHAR`'s space is.
//!
//! # Not translated (out of scope, #74)
//!
//! Composition and the candidate list: `WM_IME_STARTCOMPOSITION`, `WM_IME_COMPOSITION`, `WM_IME_ENDCOMPOSITION`,
//! `WM_IME_CHAR`, `WM_IME_NOTIFY` (the candidate window, `IMECandidate.cpp`), the status window, `enable` / `disable`'s
//! input-context calls, and `convertCharToWide` (only `WM_IME_CHAR` uses it). This manager never composes:
//! [`ImeManager::is_composing`] is always false, so the text entry's "attached and composing" branch never runs. The
//! browser's and the operating system's own IMEs still compose: what they commit arrives as characters (Bevy's
//! keyboard text), which the client gives to [`ImeManager::service_ime_message`] as `WM_CHAR`s.
//!
//! # Where this differs from upstream
//!
//! - `ImmAssociateContext` / `updateStatusWindow` in `attach` are the Win32 IME's; there is no input context here, so
//!   `attach` only remembers the window (`m_disabled` is kept, and decides nothing else).
//! - The manager lives in [`GameWindowManager::ime`] (upstream's global `TheIMEManager`), so the text entry and the
//!   shell reach it as upstream's code reaches the global; it always exists, as it does in the game once `GameClient`
//!   is initialised.
//! - The message's `windowsHandle` (asserted to be the application's) is not modelled.
//!
//! # Representation
//!
//! [`ImeManager`]: the attached window (a [`WinId`]), `m_result`, `m_disabled`.

use crate::client::window_manager::{gwm, GameWindowManager, GuiHost};
use crate::client::window_script::WinId;

/// `WM_CHAR`.
pub const WM_CHAR: u32 = 0x0102;
/// `VK_RETURN`.
pub const VK_RETURN: u32 = 0x0D;

/// `IMEManager` (the parts translated: the attached window and the `WM_CHAR` case).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImeManager {
    /// `m_window`: the window the characters go to.
    pub window: Option<WinId>,
    /// `m_result`: what the window procedure returns for a message the manager ate.
    pub result: i32,
    /// `m_disabled`.
    pub disabled: i32,
}

impl ImeManager {
    /// `attach(window)`: a new window replaces the old one (`detatch` first); the same window is left as it is.
    pub fn attach(&mut self, window: Option<WinId>) {
        if self.window != window {
            self.detatch();
            // `if (m_disabled == 0) { ImmAssociateContext(...); updateStatusWindow(); }`: no input context here
            self.window = window;
        }
    }

    /// `detatch()` (upstream's spelling): no window.
    pub fn detatch(&mut self) {
        self.window = None;
    }

    /// `isAttachedTo(window)`.
    pub fn is_attached_to(&self, window: Option<WinId>) -> bool {
        self.window == window
    }

    /// `getWindow()`.
    pub fn get_window(&self) -> Option<WinId> {
        self.window
    }

    /// `result()`.
    pub fn result(&self) -> i32 {
        self.result
    }

    /// `isComposing()`: never (composition is not translated).
    pub fn is_composing(&self) -> bool {
        false
    }
}

/// `TheIMEManager->serviceIMEMessage(hWnd, message, wParam, lParam)`, as `WndProc` calls it: whether the manager ate
/// the message. Only `WM_CHAR` is translated (any other message: `false`, as upstream's cases that return `FALSE`).
pub fn service_ime_message<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, message: u32, w_param: u32, l_param: u32) -> bool {
    if message != WM_CHAR {
        return false;
    }
    let wchar = w_param & 0xFFFF;
    let Some(w) = wm.ime.window else { return false };
    let lowest = if crate::logic::modules::plant(4709) { 33 } else { 32 };
    if !(wchar >= lowest || wchar == VK_RETURN) {
        return false;
    }
    if wm.ext.get(w).is_none_or(|e| e.freed) {
        // the window was destroyed with the focus and freed: upstream sends to freed memory
        wm.set_undefined("serviceIMEMessage WM_CHAR: the attached window was freed");
        return false;
    }
    wm.win_send_input_msg(h, Some(w), gwm::IME_CHAR, wchar, l_param);
    wm.ime.result = 0;
    true
}

/// The characters of `text` (as typed: one `WM_CHAR` per UTF-16 unit, as Windows sends a character outside the
/// Basic Multilingual Plane, a surrogate pair, as two) through [`service_ime_message`] (new: the client's glue for its
/// keyboard text events, upstream's `WndProc` for each message). How many were eaten.
pub fn type_text<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, text: &str) -> usize {
    let mut eaten = 0;
    for u in text.encode_utf16() {
        if service_ime_message(wm, h, WM_CHAR, u32::from(u), 0) {
            eaten += 1;
        }
    }
    eaten
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::gadget_text_entry::{entry_rt, gem};
    use crate::client::window_manager::tests::Rec;
    use crate::client::window_manager::{key, MsgHandled};
    use crate::client::window_script::tests::Host as WndTestHost;
    use crate::client::window_script::WindowScript;

    /// A layout: `Parent` (`TestSystem`, which records and handles) with two text entries, `E` at (10,10)-(110,30)
    /// (at most 6 characters typed: `maxTextLen` 7) and `E2` at (10,40)-(110,60), and a push button `B` at (10,70).
    pub(crate) const ENTRIES: &[u8] = b"FILE_VERSION = 2;\n\
STARTLAYOUTBLOCK\n  LAYOUTINIT = [None];\n  LAYOUTUPDATE = [None];\n  LAYOUTSHUTDOWN = [None];\nENDLAYOUTBLOCK\n\
WINDOW\n  WINDOWTYPE = USER;\n  SCREENRECT = UPPERLEFT: 0 0, BOTTOMRIGHT: 400 300, CREATIONRESOLUTION: 800 600;\n\
  NAME = \"T.wnd:Parent\";\n  STATUS = ENABLED;\n  STYLE = USER;\n  SYSTEMCALLBACK = \"TestSystem\";\n\
  CHILD\n\
  WINDOW\n    WINDOWTYPE = ENTRYFIELD;\n    SCREENRECT = UPPERLEFT: 10 10, BOTTOMRIGHT: 110 30, CREATIONRESOLUTION: 800 600;\n\
    NAME = \"T.wnd:E\";\n    STATUS = ENABLED;\n    STYLE = ENTRYFIELD;\n\
    TEXTENTRYDATA = MAXLEN: 7, SECRETTEXT: 0, NUMERICALONLY: 0, ALPHANUMERICALONLY: 0, ASCIIONLY: 0;\n\
  END\n\
  WINDOW\n    WINDOWTYPE = ENTRYFIELD;\n    SCREENRECT = UPPERLEFT: 10 40, BOTTOMRIGHT: 110 60, CREATIONRESOLUTION: 800 600;\n\
    NAME = \"T.wnd:E2\";\n    STATUS = ENABLED;\n    STYLE = ENTRYFIELD;\n\
    TEXTENTRYDATA = MAXLEN: 20, SECRETTEXT: 1, NUMERICALONLY: 0, ALPHANUMERICALONLY: 0, ASCIIONLY: 0;\n\
  END\n\
  WINDOW\n    WINDOWTYPE = PUSHBUTTON;\n    SCREENRECT = UPPERLEFT: 10 70, BOTTOMRIGHT: 60 90, CREATIONRESOLUTION: 800 600;\n\
    NAME = \"T.wnd:B\";\n    STATUS = ENABLED;\n    STYLE = PUSHBUTTON;\n\
  END\n\
  ENDALLCHILDREN\n\
END\n";

    fn load(wm: &mut GameWindowManager, h: &mut Rec) -> (WinId, WinId, WinId) {
        let mut wh = WndTestHost::default();
        wm.win_create_from_script(&mut WindowScript::new(), b"T.wnd", ENTRIES, &mut wh, h).expect("the layout reads");
        let f = |n: &str| wm.tree.find_by_name(n.as_bytes()).expect(n);
        (f("T.wnd:E"), f("T.wnd:E2"), f("T.wnd:B"))
    }

    fn text(wm: &GameWindowManager, e: WinId) -> String {
        String::from_utf16_lossy(&entry_rt(wm, e).expect("entry").text.text)
    }

    /// #74, predicted from IMEManager.cpp and GadgetTextEntry.cpp before the run:
    ///
    /// - before any click nothing is attached: `WM_CHAR` `a` is not eaten and `E` stays empty;
    /// - (the test host's display is 1600x1200: the layout is read at scale 2, `E` at (20,20)-(220,60), `E2` at
    ///   (20,80)-(220,120))
    /// - a left press on `E` (at (40,40)) gives it the focus (`GadgetTextEntryInput`'s `winSetFocus`), whose
    ///   `GWM_INPUT_FOCUS` attaches the manager to `E`;
    /// - typing `héllo wörld` (11 units) then: each unit of 32 or more is eaten, but `E` takes 6 (`maxTextLen - 1`):
    ///   `héllo `, and the owner gets 6 `GEM_UPDATE_TEXT`;
    /// - a tab (9), a backspace (8) and escape (27) as `WM_CHAR` are not eaten and change nothing;
    /// - backspace as a key (`winProcessKey(KEY_BACKSPACE, down)`, DirectInput's path) erases the space: `héllo`;
    /// - return (13) is eaten, and the owner gets `GEM_EDIT_DONE` naming `E`;
    /// - a left press on `E2` moves the focus: `E` loses it (`attach(NULL)`, as it is attached), `E2` attaches; `xy` goes
    ///   to `E2` (whose secret text is `**`), `E` keeps `héllo`;
    /// - `WM_KEYDOWN` (0x100) is never eaten; the result is 0 after an eaten character.
    #[test]
    fn typed_characters_reach_the_focused_text_entry() {
        let mut wm = GameWindowManager::new((800, 600));
        let mut h = Rec::default();
        let (e, e2, _b) = load(&mut wm, &mut h);
        assert_eq!(type_text(&mut wm, &mut h, "a"), 0);
        assert_eq!(text(&wm, e), "");
        wm.win_process_mouse_event(&mut h, gwm::LEFT_DOWN, (40, 40), (0, 0));
        assert_eq!(wm.keyboard_focus, Some(e));
        assert!(wm.ime.is_attached_to(Some(e)));
        h.sys.clear();
        wm.ime.result = 5;
        assert_eq!(type_text(&mut wm, &mut h, "héllo wörld"), 11);
        assert_eq!(wm.ime.result(), 0);
        assert_eq!(text(&wm, e), "héllo ");
        assert_eq!(h.sys.iter().filter(|s| s.2 == gem::UPDATE_TEXT).count(), 6);
        for c in [9, 8, 27] {
            assert!(!service_ime_message(&mut wm, &mut h, WM_CHAR, c, 0));
        }
        assert_eq!(text(&wm, e), "héllo ");
        assert_eq!(wm.win_process_key(&mut h, key::BACKSPACE, key::STATE_DOWN), crate::client::window_manager::WinInputReturnCode::Used);
        assert_eq!(text(&wm, e), "héllo");
        h.sys.clear();
        assert!(service_ime_message(&mut wm, &mut h, WM_CHAR, VK_RETURN, 0));
        assert!(h.sys.iter().any(|s| s.2 == gem::EDIT_DONE && s.3 == e as u32 + 1), "{:?}", h.sys);
        wm.win_process_mouse_event(&mut h, gwm::LEFT_UP, (40, 40), (0, 0));
        wm.win_process_mouse_event(&mut h, gwm::LEFT_DOWN, (40, 100), (0, 0));
        assert_eq!(wm.keyboard_focus, Some(e2));
        assert_eq!(wm.ime.get_window(), Some(e2));
        assert_eq!(type_text(&mut wm, &mut h, "xy"), 2);
        assert_eq!(text(&wm, e2), "xy");
        assert_eq!(String::from_utf16_lossy(&entry_rt(&wm, e2).expect("e2").s_text.text), "**");
        assert_eq!(text(&wm, e), "héllo");
        assert!(!service_ime_message(&mut wm, &mut h, 0x100, u32::from(b'z'), 0));
        assert_eq!(text(&wm, e2), "xy");
        assert!(!wm.ime.is_composing());
    }

    /// #74, the quirk kept: an entry destroyed with the focus stays attached (its system callback no longer runs).
    /// Predicted: after `winDestroy(E)` the manager is still attached to `E`; a character goes to it and is ignored
    /// (`winSendInputMsg` skips a destroyed window) but is eaten; once `update` has freed `E`, a character is upstream's
    /// undefined behaviour (recorded, not eaten). `detatch` (the shell's) ends it: then nothing is eaten.
    #[test]
    fn a_destroyed_entry_stays_attached_as_upstream() {
        let mut wm = GameWindowManager::new((800, 600));
        let mut h = Rec::default();
        let (e, _e2, _b) = load(&mut wm, &mut h);
        wm.win_set_focus(&mut h, Some(e));
        assert!(wm.ime.is_attached_to(Some(e)));
        wm.win_destroy(Some(e));
        assert!(wm.ime.is_attached_to(Some(e)));
        assert!(service_ime_message(&mut wm, &mut h, WM_CHAR, u32::from(b'q'), 0));
        wm.update(&mut h);
        assert!(!service_ime_message(&mut wm, &mut h, WM_CHAR, u32::from(b'q'), 0));
        assert!(wm.undefined.is_some());
        wm.undefined = None;
        wm.ime.detatch();
        assert!(!service_ime_message(&mut wm, &mut h, WM_CHAR, u32::from(b'q'), 0));
        assert!(wm.undefined.is_none());
        // attach to the same window twice: unchanged
        wm.ime.attach(Some(3));
        wm.ime.attach(Some(3));
        assert_eq!(wm.ime.get_window(), Some(3));
        assert_eq!(wm.win_send_input_msg(&mut h, None, gwm::IME_CHAR, 0, 0), MsgHandled::Ignored);
    }
}
