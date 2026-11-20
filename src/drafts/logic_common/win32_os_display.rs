//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 165,536 flag pairs, 20,000 warning boxes and 20,000 engine updates (the boxes, sleep, pump and audio are the host's) (test: tests/differential_draft_os_display.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/Common/OSDisplay.h
//!   GeneralsMD/Code/GameEngineDevice/Source/Win32Device/Common/Win32OSDisplay.cpp
//!   GeneralsMD/Code/GameEngineDevice/Source/Win32Device/Common/Win32GameEngine.cpp (see [`win32_game_engine_update`])
//!
//! The OS warning box: the game's flag set mapped to `MessageBox` flags, and the button mapped back. The box itself
//! (`MessageBoxW/A`, `SetWindowPos`) and `Win32GameEngine`'s message pump are the host's.

use crate::logic::modules::plant;

/// `OSDisplayButtonType`.
pub const OSDBT_OK: u32 = 0x0000_0001;
/// `OSDBT_CANCEL`.
pub const OSDBT_CANCEL: u32 = 0x0000_0002;
/// `OSDBT_ERROR`.
pub const OSDBT_ERROR: u32 = 0x8000_0000;
/// `OSDisplayOtherFlags`.
pub const OSDOF_SYSTEMMODAL: u32 = 0x0000_0001;
/// `OSDOF_APPLICATIONMODAL`.
pub const OSDOF_APPLICATIONMODAL: u32 = 0x0000_0002;
/// `OSDOF_TASKMODAL`.
pub const OSDOF_TASKMODAL: u32 = 0x0000_0004;
/// `OSDOF_EXCLAMATIONICON`.
pub const OSDOF_EXCLAMATIONICON: u32 = 0x0000_0008;
/// `OSDOF_INFORMATIONICON`.
pub const OSDOF_INFORMATIONICON: u32 = 0x0000_0010;
/// `OSDOF_ERRORICON` (note: 0x11, which also has the INFORMATION bit set).
pub const OSDOF_ERRORICON: u32 = 0x0000_0011;
/// `OSDOF_STOPICON` (0x12).
pub const OSDOF_STOPICON: u32 = 0x0000_0012;

/// Win32 `MB_*` values.
pub const MB_OK: u32 = 0x0000_0000;
/// `MB_OKCANCEL`.
pub const MB_OKCANCEL: u32 = 0x0000_0001;
/// `MB_ICONEXCLAMATION` (0x30).
pub const MB_ICONEXCLAMATION: u32 = 0x0000_0030;
/// `MB_ICONINFORMATION` (0x40).
pub const MB_ICONINFORMATION: u32 = 0x0000_0040;
/// `MB_ICONERROR` / `MB_ICONSTOP` (0x10).
pub const MB_ICONERROR: u32 = 0x0000_0010;
/// `MB_ICONSTOP`.
pub const MB_ICONSTOP: u32 = 0x0000_0010;
/// `MB_APPLMODAL` (0).
pub const MB_APPLMODAL: u32 = 0x0000_0000;
/// `MB_SYSTEMMODAL`.
pub const MB_SYSTEMMODAL: u32 = 0x0000_1000;
/// `MB_TASKMODAL`.
pub const MB_TASKMODAL: u32 = 0x0000_2000;
/// `IDOK`.
pub const IDOK: i32 = 1;

/// `BitTest(x, bit)`: `(x & bit) != 0`.
fn bit_test(x: u32, bit: u32) -> bool {
    (x & bit) != 0
}

/// `RTSFlagsToOSFlags(buttonFlags, otherFlags, outWindowsFlags)`: the OR of the matching `MB_*` flags. (`OSDBT_CANCEL` selects
/// `MB_OKCANCEL`; `OSDOF_ERRORICON` and `OSDOF_STOPICON` have the INFORMATION bit set, so they test true for it too, as upstream.)
pub fn rts_flags_to_os_flags(button_flags: u32, other_flags: u32) -> u32 {
    let mut out_windows_flags: u32 = 0;
    if bit_test(button_flags, OSDBT_OK) {
        out_windows_flags |= MB_OK;
    }
    if bit_test(button_flags, OSDBT_CANCEL) {
        out_windows_flags |= if plant(16941) { MB_OK } else { MB_OKCANCEL };
    }
    if bit_test(other_flags, OSDOF_SYSTEMMODAL) {
        out_windows_flags |= MB_SYSTEMMODAL;
    }
    if bit_test(other_flags, OSDOF_APPLICATIONMODAL) {
        out_windows_flags |= MB_APPLMODAL;
    }
    if bit_test(other_flags, OSDOF_TASKMODAL) {
        out_windows_flags |= if plant(16947) { MB_SYSTEMMODAL } else { MB_TASKMODAL };
    }
    if bit_test(other_flags, OSDOF_EXCLAMATIONICON) {
        out_windows_flags |= MB_ICONEXCLAMATION;
    }
    if bit_test(other_flags, OSDOF_INFORMATIONICON) {
        out_windows_flags |= MB_ICONINFORMATION;
    }
    if bit_test(other_flags, OSDOF_ERRORICON) {
        out_windows_flags |= if plant(16942) { MB_ICONINFORMATION } else { MB_ICONERROR };
    }
    if bit_test(other_flags, OSDOF_STOPICON) {
        out_windows_flags |= MB_ICONSTOP;
    }
    out_windows_flags
}

/// What `OSDisplayWarningBox` asks of the host.
pub trait OsDisplayHost {
    /// `TheGameText` exists.
    fn has_game_text(&self) -> bool;
    /// `TheGameText->fetch(label)`.
    fn fetch(&mut self, label: &[u8]) -> Vec<u16>;
    /// `MessageBoxW/A(NULL, message, prompt, flags)` (the narrow form is used when `!TheSystemIsUnicode`, after `SetWindowPos(HWND_NOTOPMOST)`).
    fn message_box(&mut self, message: &[u16], prompt: &[u16], flags: u32) -> i32;
}

/// `OSDisplayWarningBox(p, m, buttonFlags, otherFlags)`: `OSDBT_ERROR` without `TheGameText`; else the box over the fetched prompt and
/// message; `IDOK` is `OSDBT_OK`, any other answer `OSDBT_CANCEL`.
pub fn os_display_warning_box<H: OsDisplayHost>(host: &mut H, p: &[u8], m: &[u8], button_flags: u32, other_flags: u32) -> u32 {
    if !host.has_game_text() && !plant(16946) {
        return OSDBT_ERROR;
    }
    let prompt_str = host.fetch(p);
    let mesg_str = host.fetch(m);
    let windows_options_flags = rts_flags_to_os_flags(button_flags, other_flags);
    let return_result = if plant(16944) { host.message_box(&prompt_str, &mesg_str, windows_options_flags) } else { host.message_box(&mesg_str, &prompt_str, windows_options_flags) };
    if (if plant(16945) { return_result != IDOK } else if plant(16943) { return_result != 0 && return_result != 2 || return_result == 1 } else { return_result == IDOK }) {
        return OSDBT_OK;
    }
    OSDBT_CANCEL
}

/// What `Win32GameEngine::update` asks of the host.
pub trait Win32GameEngineHost {
    /// `GameEngine::update()`.
    fn base_update(&mut self);
    /// `ApplicationHWnd && ::IsIconic(ApplicationHWnd)`.
    fn is_iconic(&self) -> bool;
    /// `Sleep(5)`.
    fn sleep_ms(&mut self, ms: u32);
    /// `serviceWindowsOS()`.
    fn service_windows_os(&mut self);
    /// `TheLAN != NULL`: then `TheLAN->setIsActive(isActive()); TheLAN->update()`.
    fn update_lan_if_present(&mut self);
    /// `TheGameEngine->getQuitting() || TheGameLogic->isInInternetGame() || TheGameLogic->isInLanGame()`.
    fn must_keep_running(&self) -> bool;
    /// `TheAudio->setVolume(TheAudio->getVolume(aa), aa)` with `aa = 0x10` (wakes the audio manager after an alt-tab).
    fn rewake_audio(&mut self);
}

/// `Win32GameEngine::update()`: the base update; while the window is iconic (alt-tabbed) it sleeps 5 ms, services the OS, updates the
/// LAN, and breaks out when a multiplayer game or a quit needs the logic to keep running; after an iconic spell the audio is
/// re-woken; then one more `serviceWindowsOS()`. (`Win32GameEngine::init/reset` only call the base; the constructor sets
/// `SEM_FAILCRITICALERRORS` and the destructor restores it: OS-only.)
pub fn win32_game_engine_update<H: Win32GameEngineHost>(host: &mut H) {
    // call the engine normal update
    host.base_update();
    if host.is_iconic() {
        while host.is_iconic() {
            // We are alt-tabbed out here.  Sleep a bit, & process windows so that we can become un-alt-tabbed out.
            host.sleep_ms(if plant(16948) { 50 } else { 5 });
            host.service_windows_os();
            host.update_lan_if_present();
            // If we are running a multiplayer game, keep running the logic.
            if host.must_keep_running() && !plant(16949) {
                break; // keep running.
            }
        }
        // When we are alt-tabbed out... the MilesAudioManager seems to go into a coma sometimes: this wakes it up.
        host.rewake_audio();
    } else if plant(16950) {
        host.rewake_audio();
    }
    // allow windows to perform regular windows maintenance stuff like msgs
    if !plant(16951) {
        host.service_windows_os();
    }
}
