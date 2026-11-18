//! UNTESTED DRAFT (#202, #203, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/DownloadMenu.cpp
//!     (the file's statics, `closeDownloadWindow`, `errorCallback`, `successQuitCallback`, `successNoQuitCallback`,
//!      `DownloadManagerMunkee` (`downloadFile`, `OnError`, `OnEnd`, `OnProgressUpdate`, `OnStatusUpdate`),
//!      `DownloadMenuInit/Shutdown/Update/Input/System`)
//!
//! `DownloadManager` (the FTP engine, GameNetwork/DownloadManager.cpp) is the host's: each `DownloadManagerMunkee`
//! override here is the code that runs AFTER (or around) the base call, and the host calls it. `HRESULT` returns are
//! the base's and stay with the host.

use crate::client::gadget;
use crate::client::shell::ShellHost;
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;

/// The message box callbacks `errorCallback`, `successQuitCallback`, `successNoQuitCallback`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadMessageBoxCallback {
    Error,
    SuccessQuit,
    SuccessNoQuit,
}

/// The file's statics and `DownloadManagerMunkee`'s member (`NAMEKEY_INVALID` is 0, `NULL` is `None`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DownloadMenu {
    pub button_cancel_id: i32,
    pub static_text_size_id: i32,
    pub static_text_time_id: i32,
    pub static_text_file_id: i32,
    pub static_text_status_id: i32,
    pub progress_bar_munkee_id: i32,
    pub static_text_size: Option<WinId>,
    pub static_text_time: Option<WinId>,
    pub static_text_file: Option<WinId>,
    pub static_text_status: Option<WinId>,
    pub progress_bar_munkee: Option<WinId>,
    pub parent: Option<WinId>,
    /// `static time_t lastUpdate`, `static Int timeLeft`.
    pub last_update: i64,
    pub time_left: i32,
    /// `DownloadManagerMunkee::m_shouldQuitOnSuccess` (the constructor sets true, then false).
    pub should_quit_on_success: bool,
    /// `TheDownloadManager != NULL`.
    pub download_manager_exists: bool,
}

/// What the download menu reaches outside the window code.
pub trait DownloadMenuHost: ShellHost {
    fn download_menu(&mut self) -> &mut DownloadMenu;
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    /// `time(NULL)`.
    fn time_now(&self) -> i64;
    /// `UnicodeString::format(fmt, a, b)` for `GUI:DownloadBytesRatio`.
    fn format_bytes_ratio(&self, fmt: &[u16], bytesread: i32, totalsize: i32) -> Vec<u16>;
    /// `UnicodeString::format(fmt, h, m, s)` for `GUI:DownloadTimeLeft`.
    fn format_time_left(&self, fmt: &[u16], hour: i32, min: i32, sec: i32) -> Vec<u16>;
    /// `HandleCanceledDownload()` (MainMenuUtils.cpp).
    fn handle_canceled_download(&mut self);
    /// `MessageBoxOk(title, body, callback)`.
    fn message_box_ok(&mut self, wm: &mut GameWindowManager, title: &[u16], body: &[u16], callback: DownloadMessageBoxCallback);
    /// `getErrorString()`, `getStatusString()` of the download manager.
    fn download_error_string(&self) -> Vec<u16>;
    fn download_status_string(&self) -> Vec<u16>;
    /// `TheGameEngine->setQuitting(TRUE)`.
    fn game_engine_set_quitting(&mut self);
    /// `TheGameLogic->isInGame()`; `TheMessageStream->appendMessage(MSG_CLEAR_GAME_DATA)`.
    fn game_logic_is_in_game(&self) -> bool;
    fn message_stream_append_clear_game_data(&mut self);
    /// `delete TheDownloadManager; TheDownloadManager = NEW DownloadManagerMunkee`.
    fn download_manager_delete(&mut self);
    fn download_manager_create_munkee(&mut self);
    /// `isFileQueuedForDownload()`, `downloadNextQueuedFile()` (the base's).
    fn download_is_file_queued(&self) -> bool;
    fn download_next_queued_file(&mut self);
}

fn key_of<H: DownloadMenuHost>(h: &mut H, name: &str) -> i32 {
    ShellHost::name_to_key(h, format!("DownloadMenu.wnd:{name}").as_bytes()) as i32
}

/// `closeDownloadWindow()` (DownloadMenu.cpp:56): `menuLayout->runShutdown(); destroyWindows(); deleteInstance()`,
/// then focus to the main menu's parent.
fn close_download_window<H: DownloadMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let Some(parent) = h.download_menu().parent else { return }; // DEBUG_ASSERTCRASH(parent, ("No Parent"))
    if let Some(layout) = wm.ext[parent].layout {
        wm.layout_run(h, layout, 2); // runShutdown
        wm.layout_destroy_windows(layout);
        wm.layouts[layout].deleted = true;
    }

    let id = ShellHost::name_to_key(h, b"MainMenu.wnd:MainMenuParent") as i32;
    if let Some(main_win) = wm.win_get_window_from_id(None, id) {
        wm.win_set_focus(h, Some(main_win));
    }
}

/// `errorCallback()`.
fn error_callback<H: DownloadMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.handle_canceled_download();
    close_download_window(wm, h);
}

/// `successQuitCallback()`.
fn success_quit_callback<H: DownloadMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.game_engine_set_quitting();
    close_download_window(wm, h);
    if h.game_logic_is_in_game() {
        h.message_stream_append_clear_game_data();
    }
}

/// `successNoQuitCallback()`.
fn success_no_quit_callback<H: DownloadMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.handle_canceled_download();
    close_download_window(wm, h);
}

/// A message box's callback, by the enum the host kept (the host calls this when the OK is pressed).
pub fn download_message_box_ok_pressed<H: DownloadMenuHost>(wm: &mut GameWindowManager, h: &mut H, cb: DownloadMessageBoxCallback) {
    match cb {
        DownloadMessageBoxCallback::Error => error_callback(wm, h),
        DownloadMessageBoxCallback::SuccessQuit => success_quit_callback(wm, h),
        DownloadMessageBoxCallback::SuccessNoQuit => success_no_quit_callback(wm, h),
    }
}

/// `DownloadManagerMunkee::downloadFile(server, username, password, file, localfile, regkey, tryResume)`
/// (DownloadMenu.cpp:109) up to the base call: sets `m_shouldQuitOnSuccess` for a patch, shows the file's leaf name,
/// and returns the password the base is called with (`"-%s"` of it).
pub fn download_manager_munkee_download_file<H: DownloadMenuHost>(wm: &mut GameWindowManager, h: &mut H, file: &[u8], localfile: &[u8], password: &[u8]) -> Vec<u8> {
    if localfile.windows(8).any(|w| w == b"patches\\") {
        h.download_menu().should_quit_on_success = true;
    }

    if let Some(w) = h.download_menu().static_text_file {
        let mut bob: &[u8] = file;
        if let Some(p) = bob.iter().rposition(|&c| c == b'/') {
            bob = &bob[p + 1..];
        }
        if let Some(p) = bob.iter().rposition(|&c| c == b'\\') {
            bob = &bob[p + 1..];
        }
        let file_string: Vec<u16> = bob.iter().map(|&c| c as u16).collect();
        gadget::static_text_set_text(wm, h, Some(w), &file_string);
    }

    // password.format("-%s", password.str());
    let mut out = b"-".to_vec();
    out.extend_from_slice(password);
    out
}

/// `DownloadManagerMunkee::OnError(error)`, after `DownloadManager::OnError`.
pub fn download_manager_munkee_on_error<H: DownloadMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let title = h.fetch_text(b"GUI:DownloadErrorTitle");
    let body = h.download_error_string();
    h.message_box_ok(wm, &title, &body, DownloadMessageBoxCallback::Error);
}

/// `DownloadManagerMunkee::OnEnd()`, after `DownloadManager::OnEnd`. The host returns the base's `HRESULT`, or
/// `downloadNextQueuedFile()`'s when a file is queued.
pub fn download_manager_munkee_on_end<H: DownloadMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    if h.download_is_file_queued() {
        h.download_next_queued_file();
        return;
    }

    if h.download_menu().should_quit_on_success {
        let title = h.fetch_text(b"GUI:DownloadSuccessTitle");
        let body = h.fetch_text(b"GUI:DownloadSuccessMustQuit");
        h.message_box_ok(wm, &title, &body, DownloadMessageBoxCallback::SuccessQuit);
    } else {
        let title = h.fetch_text(b"GUI:DownloadSuccessTitle");
        let body = h.fetch_text(b"GUI:DownloadSuccess");
        h.message_box_ok(wm, &title, &body, DownloadMessageBoxCallback::SuccessNoQuit);
    }
}

/// The "time left" text: `GUI:DownloadTimeLeft` with hour, minute and second, or `GUI:DownloadUnknownTime`.
/// Note `takenMin = timeleft / 60` is NOT reduced modulo 60 upstream; kept.
fn time_left_string<H: DownloadMenuHost>(h: &H, time_left: i32) -> Vec<u16> {
    if time_left != 0 {
        // DEBUG_ASSERTCRASH(timeleft > 0, ("Time left is negative!"));
        let time_left = time_left.max(1);
        let taken_hour = time_left / 60 / 60;
        let taken_min = time_left / 60;
        let taken_sec = time_left % 60;
        let fmt = h.fetch_text(b"GUI:DownloadTimeLeft");
        h.format_time_left(&fmt, taken_hour, taken_min, taken_sec)
    } else {
        h.fetch_text(b"GUI:DownloadUnknownTime")
    }
}

/// `DownloadManagerMunkee::OnProgressUpdate(bytesread, totalsize, timetaken, timeleft)` (DownloadMenu.cpp:158),
/// after `DownloadManager::OnProgressUpdate`.
pub fn download_manager_munkee_on_progress_update<H: DownloadMenuHost>(wm: &mut GameWindowManager, h: &mut H, bytesread: i32, totalsize: i32, _timetaken: i32, timeleft: i32) {
    let d = h.download_menu().clone();
    if let Some(p) = d.progress_bar_munkee {
        // (`bytesread * 100 / totalsize`: a zero total is a division by zero upstream; modelled as 0)
        let percent = if totalsize != 0 { bytesread.wrapping_mul(100) / totalsize } else { 0 };
        gadget::progress_bar_set_progress(wm, h, p, percent);
    }

    if let Some(s) = d.static_text_size {
        let fmt = h.fetch_text(b"GUI:DownloadBytesRatio");
        let size_string = h.format_bytes_ratio(&fmt, bytesread, totalsize);
        gadget::static_text_set_text(wm, h, Some(s), &size_string);
    }

    h.download_menu().time_left = timeleft;
    if let Some(t) = d.static_text_time {
        if gadget::static_text_get_text(wm, t).is_empty() {
            // only update immediately the first time
            let now = h.time_now();
            h.download_menu().last_update = now;
            let time_string = time_left_string(h, timeleft);
            gadget::static_text_set_text(wm, h, Some(t), &time_string);
        }
    }
}

/// `DownloadManagerMunkee::OnStatusUpdate(status)`, after `DownloadManager::OnStatusUpdate`.
pub fn download_manager_munkee_on_status_update<H: DownloadMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    if let Some(s) = h.download_menu().static_text_status {
        let text = h.download_status_string();
        gadget::static_text_set_text(wm, h, Some(s), &text);
    }
}

/// `DownloadMenuInit(layout, userData)` (DownloadMenu.cpp:233).
pub fn download_menu_init<H: DownloadMenuHost>(wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    let parent_id = key_of(h, "ParentDownload");
    let parent = wm.win_get_window_from_id(None, parent_id);
    let ids = [key_of(h, "ButtonCancel"), key_of(h, "StaticTextSize"), key_of(h, "StaticTextTime"), key_of(h, "StaticTextFile"), key_of(h, "StaticTextStatus"), key_of(h, "ProgressBarMunkee")];
    let wins: Vec<Option<WinId>> = ids[1..].iter().map(|&id| wm.win_get_window_from_id(parent, id)).collect();
    {
        let d = h.download_menu();
        d.parent = parent;
        d.button_cancel_id = ids[0];
        d.static_text_size_id = ids[1];
        d.static_text_time_id = ids[2];
        d.static_text_file_id = ids[3];
        d.static_text_status_id = ids[4];
        d.progress_bar_munkee_id = ids[5];
        d.static_text_size = wins[0];
        d.static_text_time = wins[1];
        d.static_text_file = wins[2];
        d.static_text_status = wins[3];
        d.progress_bar_munkee = wins[4];
    }

    // DEBUG_ASSERTCRASH(!TheDownloadManager, ("Download manager already exists"));
    if h.download_menu().download_manager_exists {
        h.download_manager_delete();
    }
    h.download_manager_create_munkee();
    let d = h.download_menu();
    d.download_manager_exists = true;
    // DownloadManagerMunkee() {m_shouldQuitOnSuccess = true; m_shouldQuitOnSuccess = false;}
    d.should_quit_on_success = false;
}

/// `DownloadMenuShutdown(layout, userData)`.
pub fn download_menu_shutdown<H: DownloadMenuHost>(h: &mut H, _layout: usize) {
    // DEBUG_ASSERTCRASH(TheDownloadManager, ("No download manager"));
    if h.download_menu().download_manager_exists {
        h.download_manager_delete();
        h.download_menu().download_manager_exists = false;
    }

    let d = h.download_menu();
    d.static_text_size = None;
    d.static_text_time = None;
    d.static_text_file = None;
    d.static_text_status = None;
    d.progress_bar_munkee = None;
    d.parent = None;
}

/// `DownloadMenuUpdate(layout, userData)` (DownloadMenu.cpp:287).
pub fn download_menu_update<H: DownloadMenuHost>(wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    let Some(t) = h.download_menu().static_text_time else { return };
    if gadget::static_text_get_text(wm, t).is_empty() {
        return;
    }
    let now = h.time_now();
    if now <= h.download_menu().last_update {
        return;
    }
    h.download_menu().last_update = now;

    let time_left = h.download_menu().time_left;
    if time_left != 0 {
        // timeLeft = max(1, timeLeft) writes the static back
        h.download_menu().time_left = time_left.max(1);
    }
    let time_string = time_left_string(h, time_left);
    gadget::static_text_set_text(wm, h, Some(t), &time_string);
}

/// `DownloadMenuInput(window, msg, mData1, mData2)`: Escape's release presses *Cancel*.
pub fn download_menu_input<H: DownloadMenuHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR && (d1 & 0xFF) as u8 == key::ESC {
        if (d2 & 0xFF) & key::STATE_UP != 0 {
            let id = key_of(h, "ButtonCancel");
            let button = wm.win_get_window_from_id(Some(window), id);
            wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(button), id as u32);
        }
        return MsgHandled::Handled;
    }
    MsgHandled::Ignored
}

/// `DownloadMenuSystem(window, msg, mData1, mData2)`.
pub fn download_menu_system<H: DownloadMenuHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            // (break, not return: handled all the same)
            if d1 == 1 {
                wm.focus_reply = true;
            }
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            if control_id == h.download_menu().button_cancel_id {
                h.handle_canceled_download();
                close_download_window(wm, h);
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
