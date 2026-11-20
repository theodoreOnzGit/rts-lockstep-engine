//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 10,000 programs (test: tests/differential_draft_mmu.rs; the http SDK, registry, windows,
//! `Config.txt` and the DNS thread are stand-ins; `asyncGethostbyname`'s thread is the host's, untested).
//! The test found two defects, fixed (2026-11-20, #215): `configCallback` wrote the buffer BEFORE its last byte became NUL (upstream writes after), and a
//! DNS lookup that FAILED left `s_asyncDNSLookupInProgress` set (upstream's `asyncGethostbyname` clears it for any finished lookup).
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpy/MainMenuUtils.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/MainMenuUtils.cpp
//!
//! The "go online" gate of the main menu: before GameSpy starts, check EA's servers for game and map patches, fetch
//! (or reuse a cached copy of) the config and message of the day, ask the player about any patches, then call
//! `SetUpGameSpy` and push the login screen. It is a state machine over HTTP callbacks (`ghttpGet`/`ghttpHead`),
//! an asynchronous DNS lookup thread and message boxes; also the overall-stats and players-online fetches.
//!
//! # Abstraction
//!
//! The file-level statics are the fields of [`MainMenuUtils`]. GameSpy's HTTP SDK (`ghttp`), the Windows registry
//! (`FormatURLFromRegistry`, proxy, `hasWriteAccess`'s version round trip), the message boxes, the download menu,
//! the shell and `TheScriptEngine` are the [`MainMenuHost`]. Callbacks that upstream hands to the HTTP SDK or to
//! message boxes are methods here that the host calls (`*_callback`, [`MenuButton`]).
//!
//! # Upstream quirks kept
//!
//! * `configHeadCallback` decrements `checksLeftBeforeOnline` as soon as the server length equals the local file
//!   length, and if the file then fails to open it falls through to the "download the config again" request, whose
//!   `configCallback` decrements a second time.
//! * `configHeadCallback` reads `fileLen` bytes and sets `configBuffer[fileLen-1] = 0`, dropping the file's last
//!   byte; `configCallback` and `motdCallback` do the same to the downloaded buffer.
//! * The user/password of a patch URL are discarded when the URL has none ("credentials removed as per Security
//!   requirements").
//! * `asyncGethostbyname` starts its thread once; the DNS name passed to the thread is a string literal.

use crate::common::ascii_string::AsciiString;
use crate::logic::modules::plant;

use super::download_manager::QueuedDownload;
use super::persistent_storage_thread::atoi;

/// `LOOKUP_INPROGRESS`, `LOOKUP_FAILED`, `LOOKUP_SUCCEEDED`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookupState {
    /// `LOOKUP_INPROGRESS`.
    InProgress,
    /// `LOOKUP_FAILED`.
    Failed,
    /// `LOOKUP_SUCCEEDED`.
    Succeeded,
}

/// The buttons the message boxes of this file route back to the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuButton {
    /// `patchBeforeOnlineCallback`.
    PatchBeforeOnline,
    /// `noPatchBeforeOnlineCallback`.
    NoPatchBeforeOnline,
    /// `CancelPatchCheckCallbackAndReopenDropdown`.
    CancelPatchCheckAndReopenDropdown,
}

/// Which HTTP request a callback answers (`ghttpGet`/`ghttpHead` with a callback).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpRequestKind {
    /// `gamePatchCheckCallback` (game or map patch check).
    PatchCheck,
    /// `configHeadCallback`.
    ConfigHead,
    /// `configCallback`.
    Config,
    /// `motdCallback`.
    Motd,
    /// `overallStatsCallback`.
    OverallStats,
    /// `numPlayersOnlineCallback`.
    NumPlayersOnline,
}

/// The URLs `FormatURLFromRegistry` produces.
#[derive(Clone, Debug, Default)]
pub struct PatchUrls {
    /// `gameURL`.
    pub game_url: String,
    /// `mapURL`.
    pub map_url: String,
    /// `configURL`.
    pub config_url: String,
    /// `motdURL`.
    pub motd_url: String,
}

/// Everything the state machine reaches outside itself.
pub trait MainMenuHost {
    // ---- HTTP (ghttp) ----
    /// `ghttpGet(url, GHTTPFalse, callback, param)`; `run` is the `(void *)timeThroughOnline` param.
    fn http_get(&mut self, url: &str, kind: HttpRequestKind, run: i32);
    /// `ghttpHead(url, GHTTPFalse, callback, param)`.
    fn http_head(&mut self, url: &str, kind: HttpRequestKind, run: i32);
    /// `ghttpThink()`; false if it panicked/threw (upstream `catch (...)`).
    fn http_think(&mut self) -> bool;
    /// `ghttpSetProxy(proxy)`.
    fn http_set_proxy(&mut self, proxy: &str);
    /// `ghttpGetHeaders(request)` of the head request being answered.
    fn http_get_headers(&self) -> Vec<u8>;
    // ---- registry / files ----
    /// `FormatURLFromRegistry(gameURL, mapURL, configURL, motdURL)`.
    fn format_url_from_registry(&self) -> PatchUrls;
    /// `GetStringFromRegistry("", "Proxy", proxy)`.
    fn registry_proxy(&self) -> Option<String>;
    /// `hasWriteAccess()`'s file test: create/remove `PatchAccessTest.txt`; false if it cannot be created.
    fn can_create_test_file(&mut self) -> bool;
    /// `GetUnsignedIntFromRegistry("", "Version")`.
    fn registry_version_get(&self) -> Option<u32>;
    /// `SetUnsignedIntInRegistry("", "Version", val)`.
    fn registry_version_set(&mut self, val: u32) -> bool;
    /// `fopen("<user data>GeneralsOnline\\Config.txt", "wb")` + `fwrite`.
    fn write_config_file(&mut self, data: &[u8]);
    /// The saved config file's whole contents, None if it cannot be opened.
    fn read_config_file(&self) -> Option<Vec<u8>>;
    // ---- UI ----
    /// `MessageBoxOk(title, text, callback)` with the `GUI:` labels.
    fn message_box_ok(&mut self, title_label: &str, text_label: &str, on_ok: MenuButton);
    /// `MessageBoxOkCancel(title, text, ok, cancel)`.
    fn message_box_ok_cancel(&mut self, title_label: &str, text_label: &str, on_ok: MenuButton, on_cancel: MenuButton);
    /// `MessageBoxYesNo(title, text, yes, no)`.
    fn message_box_yes_no(&mut self, title_label: &str, text_label: &str, on_yes: MenuButton, on_no: MenuButton);
    /// `MessageBoxCancel(title, text, callback)`: creates the "checking for patches" window.
    fn message_box_cancel(&mut self, title_label: &str, text_label: &str, on_cancel: MenuButton);
    /// `TheWindowManager->winDestroy(onlineCancelWindow)`.
    fn destroy_online_cancel_window(&mut self);
    /// `winCreateLayout("Menus/DownloadMenu.wnd")`, `runInit()`, `hide(FALSE)`, `bringForward()`.
    fn show_download_menu(&mut self);
    /// `HandleCanceledDownload(resetDropDown)`.
    fn handle_canceled_download(&mut self, reset_drop_down: bool);
    // ---- download manager ----
    /// `TheDownloadManager != NULL`.
    fn has_download_manager(&self) -> bool;
    /// `TheDownloadManager->queueFileForDownload(...)`.
    fn download_manager_queue(&mut self, q: &QueuedDownload);
    /// `TheDownloadManager->downloadNextQueuedFile()`.
    fn download_manager_start_next(&mut self);
    // ---- going online ----
    /// `TheScriptEngine->signalUIInteract(TheShellHookNames[SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_SELECTED])`.
    fn signal_online_selected(&mut self);
    /// `SetUpGameSpy(motd, config)`.
    fn set_up_game_spy(&mut self, motd: &[u8], config: &[u8]);
    /// `TheShell->push("Menus/GameSpyLoginProfile.wnd")`.
    fn push_login_profile_menu(&mut self);
    // ---- async DNS ----
    /// `asyncGethostbyname("servserv.generals.ea.com")`: starts the lookup on the first call and polls on later
    /// ones.
    fn async_gethostbyname(&mut self, name: &str) -> LookupState;
    /// `TerminateThread(s_asyncDNSThreadHandle, 0)` if the lookup thread exists.
    fn terminate_dns_thread(&mut self);
    // ---- stats results ----
    /// `HandleOverallStats(buffer, len)`.
    fn handle_overall_stats(&mut self, buffer: &[u8]);
    /// `HandleNumPlayersOnline(n)`.
    fn handle_num_players_online(&mut self, n: i32);
}

/// The file-level state of MainMenuUtils.cpp.
#[derive(Clone, Debug)]
pub struct MainMenuUtils {
    /// `checkingForPatchBeforeGameSpy`.
    checking_for_patch_before_game_spy: bool,
    /// `checksLeftBeforeOnline`.
    checks_left_before_online: i32,
    /// `timeThroughOnline`.
    time_through_online: i32,
    /// `mustDownloadPatch`.
    must_download_patch: bool,
    /// `cantConnectBeforeOnline`.
    cant_connect_before_online: bool,
    /// `queuedDownloads`.
    queued_downloads: Vec<QueuedDownload>,
    /// `MOTDBuffer`.
    motd_buffer: Option<Vec<u8>>,
    /// `configBuffer`.
    config_buffer: Option<Vec<u8>>,
    /// `onlineCancelWindow != NULL`.
    online_cancel_window: bool,
    /// `s_asyncDNSLookupInProgress`.
    async_dns_lookup_in_progress: bool,
    /// `isHttpOk`.
    is_http_ok: bool,
}

/// The DNS name checked before the patch check.
const PATCH_SERVER_NAME: &str = "servserv.generals.ea.com";

impl MainMenuUtils {
    /// The statics' initial values: not checking, nothing pending, no buffers, no window, no lookup, HTTP ok.
    pub fn new() -> MainMenuUtils {
        MainMenuUtils {
            checking_for_patch_before_game_spy: false,
            checks_left_before_online: 0,
            time_through_online: 0,
            must_download_patch: false,
            cant_connect_before_online: false,
            queued_downloads: Vec::new(),
            motd_buffer: None,
            config_buffer: None,
            online_cancel_window: false,
            async_dns_lookup_in_progress: false,
            is_http_ok: true,
        }
    }

    /// The file-level state as words, in the order of the differential test's dump (#215, tests/differential_draft_mmu.rs): the flags, the counters,
    /// the queued downloads, the MOTD and config buffers as C strings (up to the first NUL), the cancel window, the lookup flag and `isHttpOk`.
    pub fn state_words(&self) -> Vec<u32> {
        fn put(w: &mut Vec<u32>, s: &[u8]) {
            w.push(s.len() as u32);
            w.extend(s.iter().map(|&b| b as u32));
        }
        fn c_string(b: &[u8]) -> &[u8] {
            match b.iter().position(|&c| c == 0) {
                Some(p) => &b[..p],
                None => b,
            }
        }
        let mut w = vec![
            0xD0D0D0D0,
            self.checking_for_patch_before_game_spy as u32,
            self.checks_left_before_online as u32,
            self.time_through_online as u32,
            self.must_download_patch as u32,
            self.cant_connect_before_online as u32,
            self.queued_downloads.len() as u32,
        ];
        for q in &self.queued_downloads {
            put(&mut w, q.server.str());
            put(&mut w, q.user_name.str());
            put(&mut w, q.password.str());
            put(&mut w, q.file.str());
            put(&mut w, q.local_file.str());
            put(&mut w, q.reg_key.str());
            w.push(q.try_resume as u32);
        }
        for b in [&self.motd_buffer, &self.config_buffer] {
            w.push(b.is_some() as u32);
            if let Some(b) = b {
                put(&mut w, c_string(b));
            }
        }
        w.push(self.online_cancel_window as u32);
        w.push(self.async_dns_lookup_in_progress as u32);
        w.push(self.is_http_ok as u32);
        w
    }

    /// `StartDownloadingPatches`: with nothing queued, cancels; otherwise opens the download menu, hands every
    /// queued download to the download manager (emptying the queue) and starts the first.
    pub fn start_downloading_patches<H: MainMenuHost>(&mut self, host: &mut H) {
        if self.queued_downloads.is_empty() {
            host.handle_canceled_download(!plant(20316));
            return;
        }
        host.show_download_menu();
        host.handle_canceled_download(false);
        if host.has_download_manager() {
            while !self.queued_downloads.is_empty() {
                let q = self.queued_downloads.remove(0);
                host.download_manager_queue(&q);
            }
            host.download_manager_start_next();
        }
    }

    /// A message box button of this file.
    pub fn on_button<H: MainMenuHost>(&mut self, host: &mut H, button: MenuButton) {
        match button {
            MenuButton::PatchBeforeOnline => self.start_downloading_patches(host),
            MenuButton::NoPatchBeforeOnline => self.no_patch_before_online_callback(host),
            MenuButton::CancelPatchCheckAndReopenDropdown => {
                host.handle_canceled_download(true);
                self.cancel_patch_check_callback(host);
            }
        }
    }

    /// `noPatchBeforeOnlineCallback`: drops the queue; if the patch was required (or the servers were unreachable)
    /// goes back to normal, otherwise goes online anyway.
    fn no_patch_before_online_callback<H: MainMenuHost>(&mut self, host: &mut H) {
        self.queued_downloads.clear();
        if (if plant(20320) { self.must_download_patch && self.cant_connect_before_online } else { self.must_download_patch || self.cant_connect_before_online }) {
            // go back to normal
            host.handle_canceled_download(true);
        } else {
            // clear out unneeded downloads and go on
            self.start_online(host);
        }
    }

    /// `hasWriteAccess`: can a test file be created, and can the registry `Version` value be read and written back.
    fn has_write_access<H: MainMenuHost>(&mut self, host: &mut H) -> bool {
        if !host.can_create_test_file() {
            return false;
        }
        let val = match host.registry_version_get() {
            Some(v) => v,
            None => return false,
        };
        host.registry_version_set(val)
    }

    fn destroy_cancel_window<H: MainMenuHost>(&mut self, host: &mut H) {
        if self.online_cancel_window {
            host.destroy_online_cancel_window();
            self.online_cancel_window = false;
        }
    }

    /// `startOnline`: stops patch checking; if the servers were unreachable, a message box (whose OK is
    /// `noPatchBeforeOnlineCallback`); if downloads are queued, an admin-rights error, a must-patch OK/Cancel or
    /// an optional-patch Yes/No; otherwise signals the script engine, runs `SetUpGameSpy` with the MOTD and config
    /// buffers (freeing them) and pushes the login screen.
    pub fn start_online<H: MainMenuHost>(&mut self, host: &mut H) {
        self.checking_for_patch_before_game_spy = false;
        self.destroy_cancel_window(host);

        if self.cant_connect_before_online {
            host.message_box_ok(
                "GUI:CannotConnectToServservTitle",
                "GUI:CannotConnectToServserv",
                MenuButton::NoPatchBeforeOnline,
            );
            return;
        }
        if !self.queued_downloads.is_empty() {
            if !self.has_write_access(host) {
                host.message_box_ok("GUI:Error", "GUI:MustHaveAdminRights", MenuButton::NoPatchBeforeOnline);
            } else if self.must_download_patch {
                host.message_box_ok_cancel(
                    "GUI:PatchAvailable",
                    "GUI:MustPatchForOnline",
                    MenuButton::PatchBeforeOnline,
                    MenuButton::NoPatchBeforeOnline,
                );
            } else {
                host.message_box_yes_no(
                    "GUI:PatchAvailable",
                    "GUI:CanPatchForOnline",
                    MenuButton::PatchBeforeOnline,
                    MenuButton::NoPatchBeforeOnline,
                );
            }
            return;
        }

        host.signal_online_selected();

        let motd = self.motd_buffer.take();
        let config = self.config_buffer.take();
        host.set_up_game_spy(motd.as_deref().unwrap_or(b""), config.as_deref().unwrap_or(b""));

        host.push_login_profile_menu();
    }

    /// `queuePatch(mandatory, downloadURL)`: splits `type://server[:port]/path` or
    /// `type://user:pass@server/path` with the same `nextToken` separators as upstream; a URL without credentials
    /// has its file path in the "user" slot and the credentials left empty. The local file is `patches\<name>`;
    /// a download whose local file is already queued is not added again.
    pub fn queue_patch(&mut self, _mandatory: bool, download_url: &AsciiString) {
        let mut url = download_url.clone();
        let mut success = true;

        let mut connection_type = AsciiString::default();
        success &= url.next_token(&mut connection_type, Some(b":")).unwrap_or(false);

        let mut server = AsciiString::default();
        success &= url.next_token(&mut server, Some(b":/")).unwrap_or(false);

        let mut user = AsciiString::default();
        success &= url.next_token(&mut user, Some(b":@")).unwrap_or(false);

        let mut pass = AsciiString::default();
        success &= url.next_token(&mut pass, Some(b"@/")).unwrap_or(false);

        let mut file_path = AsciiString::default();
        success &= url.next_token(&mut file_path, Some(b"")).unwrap_or(false);

        if !success && user.is_not_empty() {
            // no user/pass combo - move the file into it's proper place
            file_path = user.clone();
            user = AsciiString::default(); // LFeenanEA - Credentials removed as per Security requirements
            pass = AsciiString::default();
            success = true;
        }

        let mut file_str = file_path.clone();
        if let Some(pos) = file_path.reverse_find(b'/') {
            file_str = AsciiString::try_from_bytes(&file_path.str()[pos + 1..]).unwrap_or_default();
        }
        let mut file_name = AsciiString::try_from_bytes(if plant(20312) { b"patches/" } else { b"patches\\" }).unwrap_or_default();
        let _ = file_name.concat(file_str.str());

        if !success {
            return;
        }

        let q = QueuedDownload {
            file: file_path,
            local_file: file_name,
            password: pass,
            reg_key: AsciiString::default(),
            server,
            try_resume: true,
            user_name: user,
        };

        for e in self.queued_downloads.iter() {
            if (if plant(20311) { e.file == q.file } else { e.local_file == q.local_file }) {
                return; // don't add it if it exists already (because we can check multiple times)
            }
        }
        self.queued_downloads.push(q);
    }

    /// `motdCallback`: ignores an old run; stores the message of the day (last byte replaced by NUL, so the text is
    /// `buffer[..len-1]`), counts the check down and goes online when all are done.
    pub fn motd_callback<H: MainMenuHost>(&mut self, host: &mut H, run: i32, buffer: &[u8]) {
        if run != self.time_through_online {
            return; // Old callback being called!
        }
        let mut b = buffer.to_vec();
        if !plant(20319) {
            b.pop(); // MOTDBuffer[bufferLen-1] = 0
        }
        self.motd_buffer = Some(b);

        self.checks_left_before_online -= 1;
        if self.online_cancel_window && self.checks_left_before_online == 0 {
            self.destroy_cancel_window(host);
        }
        if self.checks_left_before_online == 0 {
            self.start_online(host);
        }
    }

    /// `configCallback`: ignores an old run; a failure or a body under 100 bytes counts the check down and marks the
    /// servers unreachable (only while checking before GameSpy); otherwise stores the config (last byte dropped),
    /// saves it to `Config.txt`, counts down and goes online when all checks are done.
    pub fn config_callback<H: MainMenuHost>(&mut self, host: &mut H, run: i32, success: bool, buffer: &[u8]) {
        if run != self.time_through_online {
            return;
        }
        self.config_buffer = None;

        if !success || (if plant(20314) { buffer.len() <= 100 } else { buffer.len() < 100 }) {
            if !self.checking_for_patch_before_game_spy {
                return;
            }
            self.checks_left_before_online -= 1;
            if self.online_cancel_window && self.checks_left_before_online == 0 {
                self.destroy_cancel_window(host);
            }
            self.cant_connect_before_online = true;
            if self.checks_left_before_online == 0 {
                self.start_online(host);
            }
            return;
        }

        let mut b = buffer.to_vec();
        if let Some(last) = b.last_mut() {
            *last = 0;
        }
        // fwrite(configBuffer, bufferLen, 1, fp): the buffer after `configBuffer[bufferLen-1] = 0`, so the file's last byte is the NUL
        // (~~the original buffer~~ **Corrected 2026-11-20 (#215, net4):** found by the differential test)
        host.write_config_file(&b);
        let mut stored = b;
        stored.pop();
        self.config_buffer = Some(stored);

        self.checks_left_before_online -= 1;
        if self.online_cancel_window && self.checks_left_before_online == 0 {
            self.destroy_cancel_window(host);
        }
        if self.checks_left_before_online == 0 {
            self.start_online(host);
        }
    }

    /// `configHeadCallback`: if the head request succeeded and the `Content-Length` header equals the saved
    /// `Config.txt`'s size, the cached config is reused (see the module doc for the double decrement when the
    /// file cannot then be opened); otherwise requests the config again.
    pub fn config_head_callback<H: MainMenuHost>(&mut self, host: &mut H, run: i32, success: bool) {
        if run != self.time_through_online {
            return;
        }
        if success {
            let headers_raw = host.http_get_headers();
            let mut headers = AsciiString::try_from_bytes(&headers_raw).unwrap_or_default();
            let mut line = AsciiString::default();
            while headers.next_token(&mut line, Some(b"\n\r")).unwrap_or(false) {
                let mut key = AsciiString::default();
                let mut val = AsciiString::default();
                let _ = line.next_token(&mut key, Some(b": "));
                let _ = line.next_token(&mut val, Some(b": \r\n"));

                if key.str() == b"Content-Length" && val.is_not_empty() {
                    let server_len = atoi(val.str());
                    let file_len = host.read_config_file().map(|f| f.len() as i32).unwrap_or(0);

                    if (if plant(20315) { server_len <= file_len } else { server_len == file_len }) {
                        // we don't need to download the MOTD again
                        self.checks_left_before_online -= 1;
                        if self.online_cancel_window && self.checks_left_before_online == 0 {
                            self.destroy_cancel_window(host);
                        }

                        self.config_buffer = None;

                        if let Some(mut file) = host.read_config_file() {
                            file.pop(); // configBuffer[fileLen-1] = 0
                            self.config_buffer = Some(file);

                            if self.checks_left_before_online == 0 {
                                self.start_online(host);
                            }
                            return;
                        }
                    }
                }
            }
        }

        // we need to download the MOTD again
        let urls = host.format_url_from_registry();
        host.http_get(&urls.config_url, HttpRequestKind::Config, run);
    }

    /// `gamePatchCheckCallback`: counts the check down; on failure (while checking before GameSpy) marks the servers
    /// unreachable and goes online if it was the last; otherwise reads each `type required url` line — a `patch`
    /// line queues the patch and, if `required` is non-zero, makes the patch mandatory — and goes online when all
    /// checks are done.
    pub fn game_patch_check_callback<H: MainMenuHost>(&mut self, host: &mut H, run: i32, success: bool, buffer: &[u8]) {
        if run != self.time_through_online {
            return;
        }
        self.checks_left_before_online -= 1;

        if !success {
            if !self.checking_for_patch_before_game_spy {
                return;
            }
            self.cant_connect_before_online = true;
            if self.checks_left_before_online == 0 {
                self.start_online(host);
            }
            return;
        }

        let text: &[u8] = match buffer.iter().position(|&b| b == 0) {
            Some(p) => &buffer[..p],
            None => buffer,
        };
        let mut message = AsciiString::try_from_bytes(text).unwrap_or_default();
        let mut line = AsciiString::default();
        while message.next_token(&mut line, Some(b"\r\n")).unwrap_or(false) {
            let mut ty = AsciiString::default();
            let mut req = AsciiString::default();
            let mut url = AsciiString::default();
            let mut ok = true;
            ok &= line.next_token(&mut ty, Some(b" ")).unwrap_or(false);
            ok &= line.next_token(&mut req, Some(b" ")).unwrap_or(false);
            ok &= line.next_token(&mut url, Some(b" ")).unwrap_or(false);
            if ok && ty.str() == b"patch" {
                self.queue_patch(atoi(req.str()) != 0, &url);
                if (if plant(20313) { atoi(req.str()) > 0 } else { atoi(req.str()) != 0 }) {
                    self.must_download_patch = true;
                }
            }
            // else if (ok && type == "server") {}
        }

        if self.checks_left_before_online == 0 {
            self.start_online(host);
        }
    }

    /// `CancelPatchCheckCallback`: abandons the check: no lookup, no pending checks, window gone, queue, MOTD and
    /// config dropped.
    pub fn cancel_patch_check_callback<H: MainMenuHost>(&mut self, host: &mut H) {
        self.async_dns_lookup_in_progress = false;
        host.handle_canceled_download(false); // don't dropdown
        self.checking_for_patch_before_game_spy = false;
        self.checks_left_before_online = 0;
        self.destroy_cancel_window(host);
        self.queued_downloads.clear();
        self.motd_buffer = None;
        self.config_buffer = None;
    }

    /// `overallStatsCallback`: hands a successful body to `HandleOverallStats`.
    pub fn overall_stats_callback<H: MainMenuHost>(&mut self, host: &mut H, success: bool, buffer: &[u8]) {
        if !success {
            return;
        }
        host.handle_overall_stats(buffer);
    }

    /// `numPlayersOnlineCallback`: the number after the last backslash of the trimmed body.
    pub fn num_players_online_callback<H: MainMenuHost>(&mut self, host: &mut H, success: bool, buffer: &[u8]) {
        if !success {
            return;
        }
        let text: &[u8] = match buffer.iter().position(|&b| b == 0) {
            Some(p) => &buffer[..p],
            None => buffer,
        };
        let mut message = AsciiString::try_from_bytes(text).unwrap_or_default();
        let _ = message.trim();
        let pos = match message.reverse_find(b'\\') {
            Some(p) => p,
            None => return,
        };
        let mut s = &message.str()[pos..];
        if !plant(20317) && !s.is_empty() && s[0] == b'\\' {
            s = &s[1..];
        }
        host.handle_num_players_online(atoi(s));
    }

    /// `CheckOverallStats`.
    pub fn check_overall_stats<H: MainMenuHost>(&mut self, host: &mut H) {
        host.http_get("http://gamestats.gamespy.com/ccgenzh/display.html", HttpRequestKind::OverallStats, 0);
    }

    /// `CheckNumPlayersOnline`.
    pub fn check_num_players_online<H: MainMenuHost>(&mut self, host: &mut H) {
        host.http_get(
            "http://launch.gamespyarcade.com/software/launch/arcadecount2.dll?svcname=ccgenzh",
            HttpRequestKind::NumPlayersOnline,
            0,
        );
    }

    /// `HTTPThinkWrapper`: while the DNS lookup is pending, polls it (failure goes online as unreachable, success
    /// starts the real patch check); then runs `ghttpThink` unless an earlier call panicked, in which case HTTP is
    /// off for good.
    pub fn http_think_wrapper<H: MainMenuHost>(&mut self, host: &mut H) {
        if self.async_dns_lookup_in_progress {
            let ret = host.async_gethostbyname(PATCH_SERVER_NAME);
            match ret {
                LookupState::Failed => {
                    // asyncGethostbyname clears s_asyncDNSLookupInProgress when it reports a finished lookup, failed or not
                    self.async_dns_lookup_in_progress = false;
                    self.cant_connect_before_online = true;
                    self.start_online(host);
                }
                LookupState::Succeeded => self.really_start_patch_check(host),
                LookupState::InProgress => {}
            }
        }
        if self.is_http_ok {
            if !host.http_think() {
                self.is_http_ok = false;
            }
        }
    }

    /// `StopAsyncDNSCheck`.
    pub fn stop_async_dns_check<H: MainMenuHost>(&mut self, host: &mut H) {
        host.terminate_dns_thread();
        self.async_dns_lookup_in_progress = false;
    }

    /// `StartPatchCheck`: starts a new run (`timeThroughOnline++`), shows the "checking for patches" box and begins
    /// the DNS lookup.
    pub fn start_patch_check<H: MainMenuHost>(&mut self, host: &mut H) {
        self.checking_for_patch_before_game_spy = true;
        self.cant_connect_before_online = false;
        if !plant(20318) {
            self.time_through_online += 1;
        }
        self.checks_left_before_online = 0;

        host.message_box_cancel(
            "GUI:CheckingForPatches",
            "GUI:CheckingForPatches",
            MenuButton::CancelPatchCheckAndReopenDropdown,
        );
        self.online_cancel_window = true;

        self.async_dns_lookup_in_progress = true;
        let ret = host.async_gethostbyname(PATCH_SERVER_NAME);
        match ret {
            LookupState::Failed => {
                self.async_dns_lookup_in_progress = false;
                self.cant_connect_before_online = true;
                self.start_online(host);
            }
            LookupState::Succeeded => self.really_start_patch_check(host),
            LookupState::InProgress => {}
        }
    }

    /// `reallyStartPatchCheck`: four checks (game patch, map patch, config head, MOTD), the proxy if configured,
    /// then the stats and players-online fetches.
    fn really_start_patch_check<H: MainMenuHost>(&mut self, host: &mut H) {
        self.checks_left_before_online = 4;
        // (the DNS lookup is over)
        self.async_dns_lookup_in_progress = false;

        let urls = host.format_url_from_registry();

        if let Some(proxy) = host.registry_proxy() {
            if !proxy.is_empty() {
                host.http_set_proxy(&proxy);
            }
        }

        // check for a patch first
        let run = self.time_through_online;
        host.http_get(&urls.game_url, HttpRequestKind::PatchCheck, run);
        host.http_get(&urls.map_url, HttpRequestKind::PatchCheck, run);
        host.http_head(&urls.config_url, HttpRequestKind::ConfigHead, run);
        host.http_get(&urls.motd_url, HttpRequestKind::Motd, run);

        // check total game stats
        self.check_overall_stats(host);

        // check the users online
        self.check_num_players_online(host);
    }
}

impl Default for MainMenuUtils {
    fn default() -> Self {
        MainMenuUtils::new()
    }
}
