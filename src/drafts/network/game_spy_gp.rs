//! UNTESTED DRAFT (#202, #207): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpyGP.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpyGP.cpp
//!
//! # EXCLUDED FROM THE UPSTREAM BUILD
//!
//! Like `GameSpyChat.cpp`, `GameSpyGP.cpp` is `Exclude_From_Build` in all Zero Hour configurations of
//! `GameEngine.dsp` (it includes the missing `GameNetwork/GameSpy.h` and uses `TheGameSpyChat`); the live GP code
//! is `BuddyThread.cpp` ([`super::buddy_thread`]). Drafted as the old callback set: the buddy message, status and
//! request callbacks only log; the error callback closes the buddy overlay on a fatal error and, if the chat is
//! still connected, offers to reconnect the profile. `GPConnectCallback`, `GameSpyUpdateBuddyOverlay` and
//! `IsGameSpyBuddy` are declared in the header but defined nowhere in this file.

use super::game_spy_overlay::{CallbackId, GameSpyOverlays, GsOverlayType, OverlayHost};

/// The id of the "reconnect the profile" callback (`buddyTryReconnect` -> `TheGameSpyChat->reconnectProfile()`).
pub const CHAT_RECONNECT_PROFILE: CallbackId = 0xFFFF_FFF0;

/// `GPErrorCallback(connection, arg, param)`: only a fatal error does anything: closes the buddy overlay and, if
/// `chat_is_connected`, asks (Yes/No) whether to reconnect, with `CHAT_RECONNECT_PROFILE` as the Yes callback
/// and no No callback. The result/error code name logging is dropped.
pub fn gp_error_callback<H: OverlayHost>(host: &mut H, overlays: &mut GameSpyOverlays, fatal: bool, chat_is_connected: bool) {
    if fatal {
        // if we're still connected to the chat server, tell the user.  He can always hit the buddy button to try
        // reconnecting.  Oh yes, also hide the buddy popup.
        overlays.game_spy_close_overlay(host, GsOverlayType::Buddy);
        if chat_is_connected {
            let t = host.game_text_fetch("GUI:GPErrorTitle");
            let m = host.game_text_fetch("GUI:GPDisconnected");
            overlays.gs_message_box_yes_no(host, &t, &m, Some(CHAT_RECONNECT_PROFILE), None);
        }
    }
}

/// `GPRecvBuddyMessageCallback`: logs only.
pub fn gp_recv_buddy_message_callback(_profile: i32, _message: &[u8]) {}

/// `GPRecvBuddyStatusCallback`: logs only (`GameSpyUpdateBuddyOverlay()` is commented out).
pub fn gp_recv_buddy_status_callback(_profile: i32, _index: i32) {}

/// `GPRecvBuddyRequestCallback`: logs only.
pub fn gp_recv_buddy_request_callback(_profile: i32, _reason: &[u8]) {}
