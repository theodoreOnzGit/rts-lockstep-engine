//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 programs (test: tests/differential_draft_overlay.rs; a callback that re-enters the
//! tracker is not representable by the draft's host, so upstream's clear-after-call quirk for such callbacks is untested);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpyOverlay.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpyOverlay.cpp
//!
//! The GameSpy UI shell pieces: a single tracked message box with its OK/Cancel (or Yes/No) callbacks, and the
//! table of nine overlay screens (player info, map select, buddy list, page, host options, join password,
//! ladder select, locale select, options) that can be opened, closed, toggled, raised and updated.
//!
//! # Abstraction
//!
//! The window manager, `WindowLayout`, `MessageBox*`, the audio event and the shell scripting hook are the
//! [`OverlayHost`]. A `GameWinMsgBoxFunc` callback is an opaque [`CallbackId`] the caller chooses; the host calls
//! the matching function when the button fires (it calls [`GameSpyOverlays::message_box_ok`] /
//! [`GameSpyOverlays::message_box_cancel`], which in turn call back to [`OverlayHost::invoke_callback`]).
//!
//! # Upstream quirks kept
//!
//! * `messageBoxOK`/`messageBoxCancel` call the stored function and only then clear it, so a function that puts up
//!   a new message box (and so stores a new callback) has that new callback cleared right after it returns.
//! * `GSMessageBoxYesNo` wires Yes to the "OK" tracker and No to the "Cancel" tracker.
//! * `GameSpyOpenOverlay(GSOVERLAY_BUDDY)` does nothing but explain when the buddy thread is not connected.

/// An opaque `GameWinMsgBoxFunc`.
use crate::logic::modules::plant;

pub type CallbackId = u32;

/// `enum GSOverlayType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GsOverlayType {
    /// `GSOVERLAY_PLAYERINFO`.
    PlayerInfo = 0,
    /// `GSOVERLAY_MAPSELECT`.
    MapSelect,
    /// `GSOVERLAY_BUDDY`.
    Buddy,
    /// `GSOVERLAY_PAGE`.
    Page,
    /// `GSOVERLAY_GAMEOPTIONS`.
    GameOptions,
    /// `GSOVERLAY_GAMEPASSWORD`.
    GamePassword,
    /// `GSOVERLAY_LADDERSELECT`.
    LadderSelect,
    /// `GSOVERLAY_LOCALESELECT`.
    LocaleSelect,
    /// `GSOVERLAY_OPTIONS`.
    Options,
}

/// `GSOVERLAY_MAX`.
pub const GSOVERLAY_MAX: usize = 9;

/// All overlay types in enum order.
pub const ALL_OVERLAYS: [GsOverlayType; GSOVERLAY_MAX] = [
    GsOverlayType::PlayerInfo,
    GsOverlayType::MapSelect,
    GsOverlayType::Buddy,
    GsOverlayType::Page,
    GsOverlayType::GameOptions,
    GsOverlayType::GamePassword,
    GsOverlayType::LadderSelect,
    GsOverlayType::LocaleSelect,
    GsOverlayType::Options,
];

/// `gsOverlays`: the `.wnd` files, in the order of [`GsOverlayType`].
pub const GS_OVERLAYS: [&str; GSOVERLAY_MAX] = [
    "Menus/PopupPlayerInfo.wnd",   // Player info (right-click)
    "Menus/WOLMapSelectMenu.wnd",  // Map select
    "Menus/WOLBuddyOverlay.wnd",   // Buddy list
    "Menus/WOLPageOverlay.wnd",    // Find/page
    "Menus/PopupHostGame.wnd",     // Hosting options (game name, password, etc)
    "Menus/PopupJoinGame.wnd",     // Joining options (password, etc)
    "Menus/PopupLadderSelect.wnd", // LadderSelect
    "Menus/PopupLocaleSelect.wnd", // Prompt for user's locale
    "Menus/OptionsMenu.wnd",       // popup options
];

/// Which button set a tracked message box has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageBoxKind {
    /// `MessageBoxOk`.
    Ok,
    /// `MessageBoxOkCancel`.
    OkCancel,
    /// `MessageBoxYesNo`.
    YesNo,
}

/// The UI the overlays drive.
pub trait OverlayHost {
    /// `MessageBoxOk/OkCancel/YesNo(title, message, messageBoxOK[, messageBoxCancel])`: creates the box; true if a
    /// window came back.
    fn create_message_box(&mut self, kind: MessageBoxKind, title: &[u16], message: &[u16]) -> bool;
    /// `TheWindowManager->winDestroy(messageBoxWindow)`.
    fn destroy_message_box(&mut self);
    /// `messageBoxWindow->winBringToTop()`.
    fn message_box_bring_to_top(&mut self);
    /// Calls the `GameWinMsgBoxFunc` the caller registered under `id`.
    fn invoke_callback(&mut self, id: CallbackId);
    /// `TheGameText->fetch(label)`.
    fn game_text_fetch(&self, label: &str) -> Vec<u16>;
    /// `TheGameSpyBuddyMessageQueue->isConnected()`.
    fn buddy_is_connected(&self) -> bool;
    /// `TheGameSpyBuddyMessageQueue->getLocalProfileID()`.
    fn buddy_local_profile_id(&self) -> i32;
    /// `AudioEventRTS("GUICommunicatorOpen")` through `TheAudio->addAudioEvent` (if there is an audio manager).
    fn play_communicator_open_sound(&mut self);
    /// `winCreateLayout(name)`, `runInit()`, `hide(FALSE)`, `bringForward()`.
    fn layout_create_and_show(&mut self, overlay: usize, wnd_file: &str);
    /// `overlayLayouts[i]->hide(FALSE); bringForward()` for an existing layout.
    fn layout_show(&mut self, overlay: usize);
    /// `overlayLayouts[i]->bringForward()`.
    fn layout_bring_forward(&mut self, overlay: usize);
    /// `runShutdown(); destroyWindows(); deleteInstance()`.
    fn layout_destroy(&mut self, overlay: usize);
    /// `overlayLayouts[i]->runUpdate()`.
    fn layout_run_update(&mut self, overlay: usize);
    /// `SignalUIInteraction(SHELL_SCRIPT_HOOK_OPTIONS_CLOSED)`.
    fn signal_options_closed(&mut self);
    /// `deleteNotificationBox()`.
    fn delete_notification_box(&mut self);
    /// `BuddyRequest(BUDDYREQUEST_RELOGIN)` to `TheGameSpyBuddyMessageQueue` (`buddyTryReconnect`).
    fn buddy_try_reconnect(&mut self);
}

/// The file-level statics of GameSpyOverlay.cpp.
#[derive(Clone, Debug, Default)]
pub struct GameSpyOverlays {
    /// `messageBoxWindow != NULL`.
    message_box_window: bool,
    /// `okFunc`.
    ok_func: Option<CallbackId>,
    /// `cancelFunc`.
    cancel_func: Option<CallbackId>,
    /// `reOpenPlayerInfoFlag`.
    re_open_player_info_flag: bool,
    /// `overlayLayouts[i] != NULL`.
    overlay_layouts: [bool; GSOVERLAY_MAX],
}

/// The callback id `GameSpyOpenOverlay` registers for the buddy "reconnect?" box: `buddyTryReconnect`.
pub const BUDDY_TRY_RECONNECT: CallbackId = u32::MAX;

impl GameSpyOverlays {
    /// The statics at start-up: no box, no callbacks, no overlays, no pending reopen.
    pub fn new() -> GameSpyOverlays {
        GameSpyOverlays::default()
    }

    /// The state as words, in the order of the differential test's dump (#215, tests/differential_draft_overlay.rs): whether a message box is up, the OK
    /// and Cancel callbacks (`0xFFFFFFFE` for none), the re-open flag and the nine layouts.
    pub fn state_words(&self) -> Vec<u32> {
        let mut w = vec![
            0xD0D0D0D0,
            self.message_box_window as u32,
            self.ok_func.unwrap_or(0xFFFF_FFFE),
            self.cancel_func.unwrap_or(0xFFFF_FFFE),
            self.re_open_player_info_flag as u32,
        ];
        w.extend(self.overlay_layouts.iter().map(|&b| b as u32));
        w
    }

    /// `messageBoxOK`: the box is gone; calls `okFunc` if set, then clears it (see the module doc).
    pub fn message_box_ok<H: OverlayHost>(&mut self, host: &mut H) {
        self.message_box_window = plant(20330) && self.message_box_window;
        if let Some(f) = self.ok_func {
            self.dispatch(host, f);
            self.ok_func = None;
        }
    }

    /// `messageBoxCancel`: the box is gone; calls `cancelFunc` if set, then clears it.
    pub fn message_box_cancel<H: OverlayHost>(&mut self, host: &mut H) {
        self.message_box_window = false;
        if let Some(f) = self.cancel_func {
            self.dispatch(host, f);
            self.cancel_func = None;
        }
    }

    fn dispatch<H: OverlayHost>(&mut self, host: &mut H, f: CallbackId) {
        if f == BUDDY_TRY_RECONNECT {
            host.buddy_try_reconnect();
        } else {
            host.invoke_callback(f);
        }
    }

    /// `ClearGSMessageBoxes`: destroys the current box and forgets both callbacks.
    pub fn clear_gs_message_boxes<H: OverlayHost>(&mut self, host: &mut H) {
        if self.message_box_window {
            host.destroy_message_box();
            self.message_box_window = false;
        }
        self.ok_func = None;
        if !plant(20321) {
            self.cancel_func = None;
        }
    }

    /// `GSMessageBoxOk(title, message, okFunc)`.
    pub fn gs_message_box_ok<H: OverlayHost>(&mut self, host: &mut H, title: &[u16], message: &[u16], new_ok_func: Option<CallbackId>) {
        self.clear_gs_message_boxes(host);
        self.message_box_window = host.create_message_box(MessageBoxKind::Ok, title, message);
        self.ok_func = new_ok_func;
    }

    /// `GSMessageBoxOkCancel(title, message, okFunc, cancelFunc)`.
    pub fn gs_message_box_ok_cancel<H: OverlayHost>(
        &mut self,
        host: &mut H,
        title: &[u16],
        message: &[u16],
        new_ok_func: Option<CallbackId>,
        new_cancel_func: Option<CallbackId>,
    ) {
        self.clear_gs_message_boxes(host);
        self.message_box_window = host.create_message_box(MessageBoxKind::OkCancel, title, message);
        self.ok_func = new_ok_func;
        self.cancel_func = new_cancel_func;
    }

    /// `GSMessageBoxYesNo(title, message, yesFunc, noFunc)`.
    pub fn gs_message_box_yes_no<H: OverlayHost>(
        &mut self,
        host: &mut H,
        title: &[u16],
        message: &[u16],
        new_yes_func: Option<CallbackId>,
        new_no_func: Option<CallbackId>,
    ) {
        self.clear_gs_message_boxes(host);
        self.message_box_window = host.create_message_box(MessageBoxKind::YesNo, title, message);
        if plant(20322) {
            self.ok_func = new_no_func;
            self.cancel_func = new_yes_func;
        } else {
            self.ok_func = new_yes_func;
            self.cancel_func = new_no_func;
        }
    }

    /// `RaiseGSMessageBox`: raises the overlays, then the message box if there is one.
    pub fn raise_gs_message_box<H: OverlayHost>(&mut self, host: &mut H) {
        if !plant(20326) {
            self.raise_overlays(host);
        }
        if !self.message_box_window {
            return;
        }
        host.message_box_bring_to_top();
    }

    /// `GameSpyOpenOverlay(overlay)`: for the buddy list, an unconnected buddy thread only gets an explanatory box
    /// (a reconnect prompt if it used to be connected, "no profile" otherwise) and no overlay; otherwise plays the
    /// communicator sound. Then shows the overlay's layout, creating it on first use.
    pub fn game_spy_open_overlay<H: OverlayHost>(&mut self, host: &mut H, overlay: GsOverlayType) {
        let i = overlay as usize;
        if overlay == GsOverlayType::Buddy {
            if !host.buddy_is_connected() {
                // not connected - is it because we were disconnected?
                if (host.buddy_local_profile_id() != 0) != plant(20328) {
                    // used to be connected
                    let t = host.game_text_fetch("GUI:GPErrorTitle");
                    let m = host.game_text_fetch("GUI:GPDisconnected");
                    self.gs_message_box_yes_no(host, &t, &m, Some(BUDDY_TRY_RECONNECT), None);
                } else {
                    // no profile
                    let t = host.game_text_fetch("GUI:GPErrorTitle");
                    let m = host.game_text_fetch("GUI:GPNoProfile");
                    self.gs_message_box_ok(host, &t, &m, None);
                }
                return;
            }
            host.play_communicator_open_sound();
        }
        if self.overlay_layouts[i] && !plant(20323) {
            host.layout_show(i);
        } else {
            self.overlay_layouts[i] = true;
            host.layout_create_and_show(i, GS_OVERLAYS[i]);
        }
    }

    /// `GameSpyCloseOverlay(overlay)`: the options overlay signals the shell script hook when it closes; any open
    /// layout is shut down, destroyed and forgotten.
    pub fn game_spy_close_overlay<H: OverlayHost>(&mut self, host: &mut H, overlay: GsOverlayType) {
        let i = overlay as usize;
        if overlay == GsOverlayType::Options && (self.overlay_layouts[i] || plant(20324)) {
            host.signal_options_closed();
        }
        if self.overlay_layouts[i] {
            host.layout_destroy(i);
            self.overlay_layouts[i] = false;
        }
    }

    /// `GameSpyIsOverlayOpen`.
    pub fn game_spy_is_overlay_open(&self, overlay: GsOverlayType) -> bool {
        self.overlay_layouts[overlay as usize]
    }

    /// `GameSpyToggleOverlay`.
    pub fn game_spy_toggle_overlay<H: OverlayHost>(&mut self, host: &mut H, overlay: GsOverlayType) {
        if self.game_spy_is_overlay_open(overlay) != plant(20325) {
            self.game_spy_close_overlay(host, overlay);
        } else {
            self.game_spy_open_overlay(host, overlay);
        }
    }

    /// `raiseOverlays`: brings every open overlay forward.
    pub fn raise_overlays<H: OverlayHost>(&mut self, host: &mut H) {
        for i in 0..GSOVERLAY_MAX {
            if self.overlay_layouts[i] {
                host.layout_bring_forward(i);
            }
        }
    }

    /// `GameSpyCloseAllOverlays`: closes every overlay and removes the notification box.
    pub fn game_spy_close_all_overlays<H: OverlayHost>(&mut self, host: &mut H) {
        for o in ALL_OVERLAYS.iter() {
            self.game_spy_close_overlay(host, *o);
        }
        // if we're shutting down the rest, chances are we don't want this popping up.
        if !plant(20329) {
            host.delete_notification_box();
        }
    }

    /// `GameSpyUpdateOverlays`: runs each open overlay's update.
    pub fn game_spy_update_overlays<H: OverlayHost>(&mut self, host: &mut H) {
        for i in 0..GSOVERLAY_MAX {
            if self.overlay_layouts[i] {
                host.layout_run_update(i);
            }
        }
    }

    /// `ReOpenPlayerInfo`: reopen it at the next check.
    pub fn re_open_player_info(&mut self) {
        self.re_open_player_info_flag = true;
    }

    /// `CheckReOpenPlayerInfo`: opens the player info overlay if a reopen was asked for.
    pub fn check_re_open_player_info<H: OverlayHost>(&mut self, host: &mut H) {
        if !self.re_open_player_info_flag {
            return;
        }
        self.game_spy_open_overlay(host, GsOverlayType::PlayerInfo);
        self.re_open_player_info_flag = plant(20327);
    }
}
