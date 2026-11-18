//! UNTESTED DRAFT (#202, #203, #214): compiles; not yet tested against the upstream C++.
//! Of this file, `obfuscate`, `GameSpyLoginPreferences`, `is_nick_okay` and `is_age_okay` are DIFFERENTIAL-TESTED (#215, tests/differential_draft_login.rs);
//! the window callbacks and everything else are still untested.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/WOLLoginMenu.cpp
//!     (the file's statics, `GameSpyLoginPreferences`, `obfuscate`, `startPings`, `shutdownComplete`,
//!      `EnableLoginControls`, `WOLLoginMenuInit/Shutdown/Update/Input/System`, `checkLogin`, `isNickOkay`,
//!      `isAgeOkay`)
//!
//! `ALLOW_NON_PROFILED_LOGIN` is not defined anywhere in the upstream tree (searched the headers), so the
//! `GameSpyLoginQuick.wnd` branches (the `#ifdef`'d `else` blocks and `buttonUseAccount`/`buttonDontUseAccount`
//! handlers) are not translated; the profile screen is the only one. The GameSpy service is gone: the peer and
//! buddy queues, `ThePinger`, `TheGameSpyInfo`, `TheGameSpyConfig`, the web browser and the file system are the host's.

use std::collections::BTreeMap;

use crate::client::gadget;
use crate::client::gadget_combo_box as cb;
use crate::client::gadget_list_box as lb;
use crate::client::gadget_text_entry as te;
use crate::client::shell::{self, ShellHost};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::client::window_transitions as wt;
use crate::logic::modules::plant;

/// `loginTimeoutInMS`.
const LOGIN_TIMEOUT_IN_MS: u32 = 10000;
/// `PREF_FILENAME`.
const PREF_FILENAME: &[u8] = b"GameSpyLogin.ini";

/// `obfuscate(in)` (WOLLoginMenu.cpp:77): XOR with "1337Munkee", the key restarting at its end and a byte equal to
/// the key byte left as it is (the key index still advances).
pub fn obfuscate(input: &[u8]) -> Vec<u8> {
    let xor: &[u8] = if plant(12683) { b"1337Munkey" } else { b"1337Munkee" };
    let key_len = if plant(12682) { xor.len() - 1 } else { xor.len() };
    // `in.str()` is a C string: the copy ends at the first NUL
    let end = input.iter().position(|&b| b == 0).unwrap_or(input.len());
    let mut buf = input[..end].to_vec();
    let mut c2 = 0usize;
    let mut c = 0usize;
    // `while (*c)`: stops at the first NUL
    while c < buf.len() && buf[c] != 0 {
        if c2 >= key_len {
            c2 = 0;
        }
        if buf[c] != xor[c2] || plant(12684) {
            // `*c = *c++ ^ *c2++;` — unsequenced in C++; the reading taken is the left side's address before the increment
            // (the shipped compiler's order; the differential build pins it with one edit, see build.rs) and the old value of both
            buf[c] ^= xor[c2];
            c += 1;
            c2 += 1;
        } else {
            c += 1;
            if !plant(12681) {
                c2 += 1;
            }
        }
    }
    buf
}

/// `class GameSpyLoginPreferences : public UserPreferences`. The base class's key/value map and file name are
/// `entries` and `filename`; reading and writing the file are the host's (`load` is handed the file's text).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameSpyLoginPreferences {
    pub filename: Vec<u8>,
    pub entries: BTreeMap<Vec<u8>, Vec<u8>>,
    email_password_map: BTreeMap<Vec<u8>, Vec<u8>>,
    email_nick_map: BTreeMap<Vec<u8>, Vec<Vec<u8>>>,
    email_date_map: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl GameSpyLoginPreferences {
    /// `GameSpyLoginPreferences()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `load(fname)` after `UserPreferences::load` filled `entries` (it returns false, and nothing else runs, when
    /// the file is missing: the caller passes `None` then).
    pub fn load(&mut self, fname: &[u8], entries: Option<BTreeMap<Vec<u8>, Vec<u8>>>) -> bool {
        self.filename = fname.to_vec();
        let Some(entries) = entries else { return false };
        self.entries = entries;
        let items: Vec<(Vec<u8>, Vec<u8>)> = self.entries.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for (key, value) in items {
            if key.starts_with(b"pass_") {
                let email = key[5..].to_vec();
                let quo_pass = quoted_printable_to_ascii(&value);
                let pass = if plant(12698) { quo_pass } else { obfuscate(&quo_pass) };
                self.email_password_map.insert(email, pass);
            }
            // (`if` not `else if`: a "pass_" key also falls to the chain below, matching none of its tests)
            if key.starts_with(b"date_") {
                let email = key[5..].to_vec();
                let date = if plant(12699) { value.clone() } else { quoted_printable_to_ascii(&value) };
                self.email_date_map.insert(email, date);
            } else if key.starts_with(b"nick_") {
                let email = key[5..].to_vec();
                // while (nicks.nextToken(&nick, ",")) m_emailNickMap[email].push_back(nick)
                for nick in value.split(|&c| c == b',').filter(|t| !t.is_empty() || plant(12700)) {
                    self.email_nick_map.entry(email.clone()).or_default().push(nick.to_vec());
                }
            }
        }
        true
    }

    /// `write()`: the file's text (the host writes it); `None` when there is no file name (upstream returns false).
    pub fn write(&self) -> Option<Vec<u8>> {
        if self.filename.is_empty() {
            return None;
        }
        let get = |k: &[u8]| self.entries.get(k).cloned().unwrap_or_default();
        let mut out: Vec<u8> = Vec::new();
        let mut line = |s: &[u8], v: &[u8]| {
            out.extend_from_slice(s);
            out.extend_from_slice(v);
            out.push(b'\n');
        };
        line(b"lastEmail = ", &get(b"lastEmail"));
        line(b"lastName = ", &get(b"lastName"));
        line(b"useProfiles = ", &get(b"useProfiles"));
        for (email, pass) in &self.email_password_map {
            let pass = if plant(12701) { pass.clone() } else { obfuscate(pass) };
            let quo_pass = ascii_to_quoted_printable(&pass);
            let mut k = b"pass_".to_vec();
            k.extend_from_slice(email);
            k.extend_from_slice(b" = ");
            line(&k, &quo_pass);
        }
        for (email, date) in &self.email_date_map {
            let d = if plant(12708) { date.clone() } else { ascii_to_quoted_printable(date) };
            let mut k = b"date_".to_vec();
            k.extend_from_slice(email);
            k.extend_from_slice(b" = ");
            line(&k, &d);
        }
        for (email, nicks_list) in &self.email_nick_map {
            let mut nicks = Vec::new();
            for n in nicks_list {
                nicks.extend_from_slice(n);
                if !plant(12702) {
                    nicks.push(b',');
                }
            }
            let mut k = b"nick_".to_vec();
            k.extend_from_slice(email);
            k.extend_from_slice(b" = ");
            line(&k, &nicks);
        }
        Some(out)
    }

    /// `getDateForEmail(email, month, date, year)`: the full date, with the three parts filled; empty (and the
    /// parts untouched) when there is none or it is not 8 characters.
    pub fn get_date_for_email(&self, email: &[u8], month: &mut Vec<u8>, date: &mut Vec<u8>, year: &mut Vec<u8>) -> Vec<u8> {
        let Some(full) = self.email_date_map.get(email) else { return Vec::new() };
        if (full.len() != 8 && !plant(12703)) || (plant(12703) && full.len() < 8) {
            return Vec::new();
        }
        *month = full[0..2].to_vec();
        *date = full[2..4].to_vec();
        *year = full[4..8].to_vec();
        full.clone()
    }

    /// `getPasswordForEmail(email)`.
    pub fn get_password_for_email(&self, email: &[u8]) -> Vec<u8> {
        self.email_password_map.get(email).cloned().unwrap_or_default()
    }

    /// `getNicksForEmail(email)`.
    pub fn get_nicks_for_email(&self, email: &[u8]) -> Vec<Vec<u8>> {
        self.email_nick_map.get(email).cloned().unwrap_or_default()
    }

    /// `addLogin(email, nick, password, date)`.
    pub fn add_login(&mut self, email: &[u8], nick: &[u8], password: &[u8], date: &[u8]) {
        let nicks = self.email_nick_map.entry(email.to_vec()).or_default();
        if !nicks.iter().any(|n| n == nick) || plant(12704) {
            nicks.push(nick.to_vec());
        }
        self.email_password_map.insert(email.to_vec(), password.to_vec());
        self.email_date_map.insert(email.to_vec(), date.to_vec());
    }

    /// `forgetLogin(email)`.
    pub fn forget_login(&mut self, email: &[u8]) {
        self.email_nick_map.remove(email);
        self.email_password_map.remove(email);
        if !plant(12705) {
            self.email_date_map.remove(email);
        }
    }

    /// `getEmails()`: the keys of the nick map, in map order.
    pub fn get_emails(&self) -> Vec<Vec<u8>> {
        if plant(12706) {
            return self.email_password_map.keys().cloned().collect();
        }
        self.email_nick_map.keys().cloned().collect()
    }
}

/// `QuotedPrintableToAsciiString(s)` (Common/QuotedPrintable.cpp): `_XY` hex escapes decoded. The verified port's
/// [`crate::common::map_cache::quoted_printable_to_ascii_string`] (the draft's own copy used `=XX`; upstream's escape
/// character is `_`: found by the differential test, #215).
fn quoted_printable_to_ascii(s: &[u8]) -> Vec<u8> {
    crate::common::map_cache::quoted_printable_to_ascii_string(s)
}

/// `AsciiStringToQuotedPrintable(s)`: the verified port's [`crate::common::map_cache::ascii_string_to_quoted_printable`].
fn ascii_to_quoted_printable(s: &[u8]) -> Vec<u8> {
    crate::common::map_cache::ascii_string_to_quoted_printable(s)
}

/// The file's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WolLoginMenu {
    pub web_browser_active: bool,
    pub use_web_browser_for_tos: bool,
    pub is_shutting_down: bool,
    pub button_pushed: bool,
    pub next_screen: Option<Vec<u8>>,
    pub login_attempt_time: u32,
    pub login_pref: Option<GameSpyLoginPreferences>,
    pub logged_in_ok: bool,
    pub parent_wol_login_id: i32,
    pub button_back_id: i32,
    pub button_login_id: i32,
    pub button_create_account_id: i32,
    pub button_tos_id: i32,
    pub parent_tos_id: i32,
    pub button_tos_ok_id: i32,
    pub listbox_tos_id: i32,
    pub combo_box_email_id: i32,
    pub combo_box_login_name_id: i32,
    pub text_entry_password_id: i32,
    pub check_box_remember_password_id: i32,
    pub text_entry_month_id: i32,
    pub text_entry_day_id: i32,
    pub text_entry_year_id: i32,
    pub parent_wol_login: Option<WinId>,
    pub button_back: Option<WinId>,
    pub button_login: Option<WinId>,
    pub button_create_account: Option<WinId>,
    pub button_tos: Option<WinId>,
    pub parent_tos: Option<WinId>,
    pub button_tos_ok: Option<WinId>,
    pub listbox_tos: Option<WinId>,
    pub combo_box_email: Option<WinId>,
    pub combo_box_login_name: Option<WinId>,
    pub text_entry_password: Option<WinId>,
    pub check_box_remember_password: Option<WinId>,
    pub text_entry_month: Option<WinId>,
    pub text_entry_day: Option<WinId>,
    pub text_entry_year: Option<WinId>,
}

/// The `PeerResponse` types `WOLLoginMenuUpdate` acts on.
#[derive(Clone, Debug)]
pub enum LoginPeerResponse {
    GroupRoom { group_id: i32, max_waiting: i32, name: Vec<u8>, num_games: i32, num_playing: i32, num_waiting: i32 },
    Login { nick: Vec<u8>, profile_id: i32, internal_ip: u32, external_ip: u32 },
    Disconnect { reason: i32 },
    Other,
}

/// What the login menu reaches in the GameSpy layer and the engine.
pub trait WolLoginMenuHost: ShellHost {
    fn wol_login_menu(&mut self) -> &mut WolLoginMenu;
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    fn gamespy_color_default(&self) -> i32;
    /// `UserPreferences::load(fname)`: the file's entries, or `None` when the file is missing.
    fn user_prefs_load(&mut self, fname: &[u8]) -> Option<BTreeMap<Vec<u8>, Vec<u8>>>;
    /// Writes the text `GameSpyLoginPreferences::write` made to `fname`.
    fn user_prefs_write_file(&mut self, fname: &[u8], text: &[u8]);
    /// `OptionPreferences::getBool("SawTOS", TRUE)`; `optionPref["SawTOS"] = "yes"; write()`.
    fn option_prefs_saw_tos(&mut self) -> bool;
    fn option_prefs_set_saw_tos_and_write(&mut self);
    /// `timeGetTime()`.
    fn time_get_time(&self) -> u32;
    // GameSpy
    fn tear_down_game_spy(&mut self);
    /// `SetUpGameSpy(motd, config)` with `TheGameSpyInfo`'s current MOTD and config (read before the tear down).
    fn tear_down_and_set_up_game_spy_keeping_motd_and_config(&mut self);
    fn has_gamespy_info(&self) -> bool;
    fn has_peer_message_queue(&self) -> bool;
    fn peer_next_response(&mut self) -> Option<LoginPeerResponse>;
    fn gamespy_add_group_room(&mut self, group_id: i32, max_waiting: i32, name: &[u8], num_games: i32, num_playing: i32, num_waiting: i32);
    fn gamespy_on_login(&mut self, nick: &[u8], profile_id: i32, internal_ip: u32, external_ip: u32);
    /// `BuddyRequest` of `BUDDYREQUEST_LOGIN` / `BUDDYREQUEST_LOGINNEW` (hasFirewall true), and the three
    /// `setLocalBaseName/Email/Password`.
    fn buddy_request_login(&mut self, new_account: bool, nick: &[u8], email: &[u8], password: &[u8]);
    // pings
    fn pinger_exists(&self) -> bool;
    fn pinger_get_response(&mut self) -> bool;
    fn pinger_are_pings_in_progress(&self) -> bool;
    fn pinger_get_ping_string(&mut self, timeout: i32) -> Vec<u8>;
    /// `startPings()`: `TheGameSpyConfig`'s ping servers each queued with its repetitions and timeout.
    fn start_pings(&mut self);
    fn gamespy_set_ping_string(&mut self, s: &[u8]);
    fn gamespy_clear_group_room_list(&mut self);
    /// `SignalUIInteraction(SHELL_SCRIPT_HOOK_GENERALS_ONLINE_LOGIN)`.
    fn signal_ui_interaction_generals_online_login(&mut self);
    /// After login: `GameSpyMiscPreferences::getCachedStats()` parsed with `parsePlayerKVPairs`, id set to the local
    /// profile id, stored with `setCachedLocalPlayerStats`.
    fn gamespy_store_cached_local_player_stats_from_misc_prefs(&mut self);
    fn gamespy_misc_prefs_max_messages_per_update(&mut self) -> i32;
    fn gamespy_set_max_messages_per_update(&mut self, n: i32);
    fn gamespy_load_saved_ignore_list_and_read_additional_disconnects(&mut self);
    // messages
    fn raise_gs_message_box(&mut self);
    fn gs_message_box_ok(&mut self, wm: &mut GameWindowManager, title: &[u16], body: &[u16]);
    // TOS
    fn has_web_browser(&self) -> bool;
    fn web_browser_create_window(&mut self, name: &[u8], window: Option<WinId>);
    fn web_browser_close_window(&mut self, window: Option<WinId>);
    /// The lines of `Data\\<language>\\TOS.txt` (as `asciiBuf.nextToken(&line, "\\r\\n")` yields them, after the two
    /// leading bytes are skipped), or `None` when the file did not open or was not read whole (`size > 2`).
    fn tos_file_lines(&mut self) -> Option<Vec<Vec<u8>>>;
    /// `MultiByteToWideCharSingleLine(line)`.
    fn multi_byte_to_wide_char(&self, line: &[u8]) -> Vec<u16>;
    /// `GetDateFormat(... "yyyy"/"MM"/"dd")` of today.
    fn today_year_month_day(&self) -> (i32, i32, i32);
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn ascii(w: &[u16]) -> Vec<u8> {
    w.iter().map(|&c| c as u8).collect() // AsciiString::translate
}

fn key_of<H: WolLoginMenuHost>(h: &mut H, n: &str) -> i32 {
    ShellHost::name_to_key(h, format!("GameSpyLoginProfile.wnd:{n}").as_bytes()) as i32
}

/// `shutdownComplete(layout)` (WOLLoginMenu.cpp:290).
fn shutdown_complete<H: WolLoginMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    h.wol_login_menu().is_shutting_down = false;

    wm.layout_hide(layout, true);
    let has_next = h.wol_login_menu().next_screen.is_some();
    shell::shutdown_complete(wm, h, Some(layout), has_next);

    // the login preferences are written in both cases, then freed
    if let Some(pref) = h.wol_login_menu().login_pref.take() {
        if let Some(text) = pref.write() {
            h.user_prefs_write_file(&pref.filename, &text);
        }
    }
    if let Some(next) = h.wol_login_menu().next_screen.take() {
        shell::push(wm, h, &next, false);
    }
}

/// `EnableLoginControls(state)`.
pub fn enable_login_controls<H: WolLoginMenuHost>(wm: &mut GameWindowManager, h: &mut H, state: bool) {
    let m = h.wol_login_menu().clone();
    for w in [m.button_login, m.button_create_account, m.combo_box_email, m.combo_box_login_name, m.text_entry_password, m.check_box_remember_password, m.button_tos, m.text_entry_month, m.text_entry_day, m.text_entry_year].into_iter().flatten() {
        wm.win_enable(w, state);
    }
}

/// Fills the password, nick list and date entries for `email` (the shared body of the e-mail combo box's
/// `GCM_UPDATE_TEXT` and `GCM_SELECTED` handlers).
fn fill_for_email<H: WolLoginMenuHost>(wm: &mut GameWindowManager, h: &mut H, email: &[u8]) {
    let m = h.wol_login_menu().clone();
    let pref = m.login_pref.clone().unwrap_or_default();
    let pass: Vec<u16> = pref.get_password_for_email(email).iter().map(|&c| c as u16).collect();
    te::set_text(wm, h, m.text_entry_password, &pass);
    let cached_nicks = pref.get_nicks_for_email(email);
    let mut selected_pos = -1;
    if let Some(c) = m.combo_box_login_name {
        cb::reset(wm, h, c);
        let color = h.gamespy_color_default();
        for n in &cached_nicks {
            let uni: Vec<u16> = n.iter().map(|&c| c as u16).collect();
            cb::add_entry(wm, h, c, &uni, color);
            selected_pos = 0;
        }
        if selected_pos >= 0 {
            cb::set_selected_pos(wm, h, c, selected_pos, false);
        }
    }
    if selected_pos >= 0 {
        if let Some(r) = m.check_box_remember_password {
            gadget::check_box_set_checked(wm, h, r, true);
        }
        let (mut month, mut day, mut year) = (Vec::new(), Vec::new(), Vec::new());
        pref.get_date_for_email(email, &mut month, &mut day, &mut year);
        let w = |v: &Vec<u8>| -> Vec<u16> { v.iter().map(|&c| c as u16).collect() };
        te::set_text(wm, h, m.text_entry_month, &w(&month));
        te::set_text(wm, h, m.text_entry_day, &w(&day));
        te::set_text(wm, h, m.text_entry_year, &w(&year));
    } else {
        if let Some(r) = m.check_box_remember_password {
            gadget::check_box_set_checked(wm, h, r, false);
        }
        te::set_text(wm, h, m.text_entry_month, &[]);
        te::set_text(wm, h, m.text_entry_day, &[]);
        te::set_text(wm, h, m.text_entry_year, &[]);
    }
}

/// `WOLLoginMenuInit(layout, userData)` (WOLLoginMenu.cpp:352).
pub fn wol_login_menu_init<H: WolLoginMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    {
        let m = h.wol_login_menu();
        m.next_screen = None;
        m.button_pushed = false;
        m.is_shutting_down = false;
        m.login_attempt_time = 0;
    }

    if h.wol_login_menu().login_pref.is_none() {
        let mut pref = GameSpyLoginPreferences::new();
        let entries = h.user_prefs_load(PREF_FILENAME);
        pref.load(PREF_FILENAME, entries);
        h.wol_login_menu().login_pref = Some(pref);
    }

    // hide the ESRB block when its title text is empty (shorter than 2 characters)
    let esrb_title_id = key_of(h, "StaticTextESRBTop");
    let esrb_parent_id = key_of(h, "ParentESRB");
    let esrb_title = wm.win_get_window_from_id(None, esrb_title_id);
    let esrb_parent = wm.win_get_window_from_id(None, esrb_parent_id);
    if let (Some(t), Some(p)) = (esrb_title, esrb_parent) {
        if gadget::static_text_get_text(wm, t).len() < 2 {
            wm.win_hide(p, true);
        }
    }

    let names = ["WOLLoginMenuParent", "ButtonBack", "ButtonLogin", "ButtonCreateAccount", "ButtonTOS", "ParentTOS", "ButtonTOSOK", "ListboxTOS", "ComboBoxEmail", "ComboBoxLoginName", "TextEntryPassword", "CheckBoxRememberInfo", "TextEntryMonth", "TextEntryDay", "TextEntryYear"];
    let ids: Vec<i32> = names.iter().map(|n| key_of(h, n)).collect();
    let w: Vec<Option<WinId>> = ids.iter().map(|&id| wm.win_get_window_from_id(None, id)).collect();
    {
        let m = h.wol_login_menu();
        m.parent_wol_login_id = ids[0];
        m.button_back_id = ids[1];
        m.button_login_id = ids[2];
        m.button_create_account_id = ids[3];
        m.button_tos_id = ids[4];
        m.parent_tos_id = ids[5];
        m.button_tos_ok_id = ids[6];
        m.listbox_tos_id = ids[7];
        m.combo_box_email_id = ids[8];
        m.combo_box_login_name_id = ids[9];
        m.text_entry_password_id = ids[10];
        m.check_box_remember_password_id = ids[11];
        m.text_entry_month_id = ids[12];
        m.text_entry_day_id = ids[13];
        m.text_entry_year_id = ids[14];
        m.parent_wol_login = w[0];
        m.button_back = w[1];
        m.button_login = w[2];
        m.button_create_account = w[3];
        m.button_tos = w[4];
        m.parent_tos = w[5];
        m.button_tos_ok = w[6];
        m.listbox_tos = w[7];
        m.combo_box_email = w[8];
        m.combo_box_login_name = w[9];
        m.text_entry_password = w[10];
        m.check_box_remember_password = w[11];
        m.text_entry_month = w[12];
        m.text_entry_day = w[13];
        m.text_entry_year = w[14];
    }
    let m = h.wol_login_menu().clone();

    te::set_text(wm, h, m.text_entry_month, &[]);
    te::set_text(wm, h, m.text_entry_day, &[]);
    te::set_text(wm, h, m.text_entry_year, &[]);

    // the tab order: email first, then the rest
    let tab_list: Vec<WinId> = [m.combo_box_email, m.combo_box_login_name, m.text_entry_password, m.text_entry_month, m.text_entry_day, m.text_entry_year, m.check_box_remember_password, m.button_login, m.button_create_account, m.button_tos, m.button_back].into_iter().flatten().collect();
    wm.clear_tab_list();
    wm.register_tab_list(tab_list);
    wm.win_set_focus(h, m.combo_box_email);

    // (the profile screen is the only one: GameSpyUseProfiles is implied)
    if let Some(c) = m.combo_box_email {
        cb::reset(wm, h, c);
    }
    te::set_text(wm, h, m.text_entry_password, &[]);

    let pref = m.login_pref.clone().unwrap_or_default();
    let last_name = pref.entries.get(&b"lastName"[..]).cloned().unwrap_or_default();
    let last_email = pref.entries.get(&b"lastEmail"[..]).cloned().unwrap_or_default();
    let mut mark_check_box = false;

    let color = h.gamespy_color_default();
    let cached_emails = pref.get_emails();
    let mut selected_pos = -1;
    for e in &cached_emails {
        let uni: Vec<u16> = e.iter().map(|&c| c as u16).collect();
        if let Some(c) = m.combo_box_email {
            let pos = cb::add_entry(wm, h, c, &uni, color);
            if *e == last_email {
                selected_pos = pos;
            }
        }
    }
    if selected_pos >= 0 {
        if let Some(c) = m.combo_box_email {
            cb::set_selected_pos(wm, h, c, selected_pos, false);
        }
        let pass: Vec<u16> = pref.get_password_for_email(&last_email).iter().map(|&c| c as u16).collect();
        te::set_text(wm, h, m.text_entry_password, &pass);
        let (mut month, mut day, mut year) = (Vec::new(), Vec::new(), Vec::new());
        pref.get_date_for_email(&last_email, &mut month, &mut day, &mut year);
        let w16 = |v: &Vec<u8>| -> Vec<u16> { v.iter().map(|&c| c as u16).collect() };
        te::set_text(wm, h, m.text_entry_month, &w16(&month));
        te::set_text(wm, h, m.text_entry_day, &w16(&day));
        te::set_text(wm, h, m.text_entry_year, &w16(&year));
        mark_check_box = true;
    }

    if let Some(c) = m.combo_box_login_name {
        cb::reset(wm, h, c);
    }
    let cached_nicks = pref.get_nicks_for_email(&last_email);
    selected_pos = -1;
    for n in &cached_nicks {
        let uni: Vec<u16> = n.iter().map(|&c| c as u16).collect();
        if let Some(c) = m.combo_box_login_name {
            let pos = cb::add_entry(wm, h, c, &uni, color);
            if *n == last_name {
                selected_pos = pos;
            }
        }
    }
    if selected_pos >= 0 {
        if let Some(c) = m.combo_box_login_name {
            cb::set_selected_pos(wm, h, c, selected_pos, false);
        }
        mark_check_box = true;
    }

    if let Some(r) = m.check_box_remember_password {
        gadget::check_box_set_checked(wm, h, r, mark_check_box);
    }

    enable_login_controls(wm, h, true);

    // Show Menu
    wm.layout_hide(layout, false);
    h.raise_gs_message_box();

    if !h.option_prefs_saw_tos() {
        wm.win_send_system_msg(h, m.parent_wol_login, ggm::GBM_SELECTED, win_data(m.button_tos), m.button_tos_id as u32);
    }

    wt::set_group(wm, h, b"GameSpyLoginProfileFade", false);
}

/// `WOLLoginMenuShutdown(layout, userData)`; `pop_immediate` is `*(Bool *)userData`.
pub fn wol_login_menu_shutdown<H: WolLoginMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize, pop_immediate: bool) {
    h.wol_login_menu().is_shutting_down = true;
    h.wol_login_menu().logged_in_ok = false;

    wm.clear_tab_list();

    if h.wol_login_menu().web_browser_active {
        if h.has_web_browser() {
            let l = h.wol_login_menu().listbox_tos;
            h.web_browser_close_window(l);
        }
        h.wol_login_menu().web_browser_active = false;
    }

    // if we are doing an immediate pop, just do it
    if pop_immediate {
        shutdown_complete(wm, h, layout);
        return;
    }

    shell::reverse_animate_window(wm, h);
    wt::reverse(wm, h, b"GameSpyLoginProfileFade");
}

/// `checkLogin()`.
fn check_login<H: WolLoginMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    if h.wol_login_menu().logged_in_ok && h.pinger_exists() && !h.pinger_are_pings_in_progress() {
        let ping_str = h.pinger_get_ping_string(1000);
        h.gamespy_set_ping_string(&ping_str);
        h.wol_login_menu().button_pushed = true;
        h.wol_login_menu().logged_in_ok = false; // don't try this again
        h.wol_login_menu().login_attempt_time = 0;
        h.gamespy_clear_group_room_list();

        h.signal_ui_interaction_generals_online_login();

        h.wol_login_menu().next_screen = Some(b"Menus/WOLWelcomeMenu.wnd".to_vec());
        shell::pop(wm, h);

        h.gamespy_store_cached_local_player_stats_from_misc_prefs();
    }
}

/// The error box for a failed or timed-out connection, then the login controls back on and the GameSpy layer rebuilt
/// keeping its MOTD and config.
fn login_failed_box<H: WolLoginMenuHost>(wm: &mut GameWindowManager, h: &mut H, reason_label: &str) {
    let title = h.fetch_text(b"GUI:GSErrorTitle");
    let body = h.fetch_text(reason_label.as_bytes());
    h.gs_message_box_ok(wm, &title, &body);
    enable_login_controls(wm, h, true);
    h.tear_down_and_set_up_game_spy_keeping_motd_and_config();
}

/// `WOLLoginMenuUpdate(layout, userData)` (WOLLoginMenu.cpp:659).
pub fn wol_login_menu_update<H: WolLoginMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    if h.wol_login_menu().is_shutting_down && shell::is_anim_finished(wm, h) && wt::is_finished(wm) {
        shutdown_complete(wm, h, layout);
    }

    if shell::is_anim_finished(wm, h) && !h.wol_login_menu().button_pushed && h.has_peer_message_queue() {
        if h.pinger_exists() && h.pinger_get_response() {
            check_login(wm, h);
        }

        if !h.wol_login_menu().logged_in_ok {
            if let Some(resp) = h.peer_next_response() {
                match resp {
                    LoginPeerResponse::GroupRoom { group_id, max_waiting, name, num_games, num_playing, num_waiting } => {
                        // (room.m_translatedName = L"TEST" upstream)
                        h.gamespy_add_group_room(group_id, max_waiting, &name, num_games, num_playing, num_waiting);
                    }
                    LoginPeerResponse::Login { nick, profile_id, internal_ip, external_ip } => {
                        h.wol_login_menu().logged_in_ok = true;
                        h.gamespy_on_login(&nick, profile_id, internal_ip, external_ip);
                        h.gamespy_load_saved_ignore_list_and_read_additional_disconnects();
                        let n = h.gamespy_misc_prefs_max_messages_per_update();
                        h.gamespy_set_max_messages_per_update(n);
                    }
                    LoginPeerResponse::Disconnect { reason } => {
                        h.wol_login_menu().login_attempt_time = 0;
                        login_failed_box(wm, h, &format!("GUI:GSDisconReason{reason}"));
                    }
                    LoginPeerResponse::Other => {}
                }
            }
        }
        check_login(wm, h);
    }

    let t = h.wol_login_menu().login_attempt_time;
    if h.has_gamespy_info() && !h.wol_login_menu().button_pushed && t != 0 && t.wrapping_add(LOGIN_TIMEOUT_IN_MS) < h.time_get_time() {
        // Timed out
        h.wol_login_menu().login_attempt_time = 0;
        login_failed_box(wm, h, "GUI:GSDisconReason4"); // ("could not connect to server")
    }
}

/// `WOLLoginMenuInput(window, msg, mData1, mData2)`.
pub fn wol_login_menu_input<H: WolLoginMenuHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR {
        if h.wol_login_menu().button_pushed {
            return MsgHandled::Ignored;
        }
        if (d1 & 0xFF) as u8 == key::ESC {
            if (d2 & 0xFF) & key::STATE_UP != 0 {
                let m = h.wol_login_menu().clone();
                wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(m.button_back), m.button_back_id as u32);
            }
            return MsgHandled::Handled;
        }
    }
    MsgHandled::Ignored
}

/// `isNickOkay(nick)`: the last typed character must be a legal IRC nick character; "-" alone is not allowed.
pub fn is_nick_okay(nick: &[u16]) -> bool {
    let legal_irc_chars = if plant(12688) {
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789[]`_^|}-"
    } else {
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789[]`_^{|}-"
    };
    let len = nick.len();
    if len == 0 {
        return !plant(12686);
    }
    if len == 1 && nick[0] == '-' as u16 && !plant(12687) {
        return false;
    }
    let new_char = if plant(12685) { nick[0] } else { nick[len - 1] };
    legal_irc_chars.encode_utf16().any(|c| c == new_char)
}

/// `atoi(s)`: optional leading white space (space, `\t`..`\r`) and sign, then digits, stopping at the first other byte; no digits is 0;
/// a value outside `Int` wraps (the 64-bit `strtol` of glibc, truncated: tested up to 18 digits; the CRT's own on 32 bits wraps
/// likewise). The draft's first version failed to parse such a value and gave 0, and took Unicode white space.
fn atoi(s: &[u8]) -> i32 {
    let mut i = 0;
    while i < s.len() && (s[i] == b' ' || (b'\t'..=b'\r').contains(&s[i])) {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        neg = s[i] == b'-';
        i += 1;
    }
    let mut v: i64 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = v.wrapping_mul(10).wrapping_add(i64::from(s[i] - b'0'));
        i += 1;
    }
    (if neg { v.wrapping_neg() } else { v }) as i32
}

/// `format("%02.2d", n)`: at least two digits (the precision), a `-` first when negative (the `0` flag is ignored with a
/// precision, and the width of 2 is already met).
fn format_02_2d(n: i32) -> Vec<u8> {
    let m = i64::from(n);
    if n < 0 {
        format!("-{:02}", -m).into_bytes()
    } else {
        format!("{m:02}").into_bytes()
    }
}

/// `isAgeOkay(month, day, year)` (WOLLoginMenu.cpp:802): the user must be at least 13 today; `today` is the system
/// date (`GetDateFormat`). `month` and `day` are rewritten as two digits.
pub fn is_age_okay(month: &mut Vec<u8>, day: &mut Vec<u8>, year: &[u8], today: (i32, i32, i32)) -> bool {
    if month.is_empty() || day.is_empty() || year.is_empty() || (year.len() != 4 && !plant(12696)) {
        return false;
    }
    let month_int = atoi(month);
    let day_int = atoi(day);
    if (month_int > 12 && !plant(12694)) || (plant(12694) && month_int >= 12) || (day_int > 31 && !plant(12695)) || (plant(12695) && day_int >= 31) {
        return false;
    }
    *month = if plant(12697) { month_int.to_string().into_bytes() } else { format_02_2d(month_int) };
    *day = format_02_2d(day_int);

    let (sys_year, sys_month, sys_day) = today;
    let user_year = atoi(year);
    if (sys_year.wrapping_sub(user_year) >= 14 && !plant(12690)) || (plant(12690) && sys_year.wrapping_sub(user_year) > 14) {
        return true;
    } else if (sys_year.wrapping_sub(user_year) <= 12 && !plant(12691)) || (plant(12691) && sys_year.wrapping_sub(user_year) < 12) {
        return false;
    }
    // exactly 13 years: compare the month, then the day
    let user_month = atoi(month);
    if (sys_month.wrapping_sub(user_month) > 0 && !plant(12692)) || (plant(12692) && sys_month.wrapping_sub(user_month) >= 0) {
        return true;
    } else if sys_month.wrapping_sub(user_month) < 0 {
        return false;
    }
    let user_day = atoi(day);
    if (sys_day.wrapping_sub(user_day) < 0 && !plant(12693)) || (plant(12693) && sys_day.wrapping_sub(user_day) <= 0) {
        return false;
    }
    true
}

/// Trims leading/trailing white space and one trailing `\` and one trailing `/` (the `GCM_UPDATE_TEXT` clean-up).
fn trim_nick_or_email(u: &[u16]) -> Vec<u16> {
    let mut t = crate::client::replay_menu::trim(u);
    if !t.is_empty() && t[t.len() - 1] == '\\' as u16 {
        t.pop();
    }
    if !t.is_empty() && t[t.len() - 1] == '/' as u16 {
        t.pop();
    }
    t
}

/// The "no login info" box for the missing combination of fields (used by both buttons).
fn no_login_info_box<H: WolLoginMenuHost>(wm: &mut GameWindowManager, h: &mut H, title_label: &[u8], email: bool, login: bool, password: bool) {
    // `email`, `login`, `password`: whether each is EMPTY
    let label: &[u8] = if email && login && password {
        b"GUI:GSNoLoginInfoAll"
    } else if email && login {
        b"GUI:GSNoLoginInfoEmailNickname"
    } else if email && password {
        b"GUI:GSNoLoginInfoEmailPassword"
    } else if login && password {
        b"GUI:GSNoLoginInfoNicknamePassword"
    } else if email {
        b"GUI:GSNoLoginInfoEmail"
    } else if password {
        b"GUI:GSNoLoginInfoPassword"
    } else if login {
        b"GUI:GSNoLoginInfoNickname"
    } else {
        b"GUI:GSNoLoginInfoAll"
    };
    let title = h.fetch_text(title_label);
    let body = h.fetch_text(label);
    h.gs_message_box_ok(wm, &title, &body);
}

/// `WOLLoginMenuSystem(window, msg, mData1, mData2)` (WOLLoginMenu.cpp:870).
pub fn wol_login_menu_system<H: WolLoginMenuHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            // if we're givin the opportunity to take the keyboard focus we must say we want it
            if d1 == 1 {
                wm.focus_reply = true;
            }
            return MsgHandled::Handled;
        }
        cb::gcm::UPDATE_TEXT => {
            let m = h.wol_login_menu().clone();
            let u_nick = m.combo_box_login_name.map(|c| cb::get_text(wm, h, c)).unwrap_or_default();
            let u_email = m.combo_box_email.map(|c| cb::get_text(wm, h, c)).unwrap_or_default();
            let nick = ascii(&u_nick);
            let email = ascii(&u_email);

            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;

            let trimmed_nick = trim_nick_or_email(&u_nick);
            let trimmed_email = trim_nick_or_email(&u_email);

            if trimmed_email.len() != u_email.len() {
                if let Some(c) = m.combo_box_email {
                    cb::set_text(wm, h, c, &trimmed_email);
                }
                return MsgHandled::Handled;
            }
            if trimmed_nick.len() != nick.len() {
                if let Some(c) = m.combo_box_login_name {
                    cb::set_text(wm, h, c, &trimmed_nick);
                }
                return MsgHandled::Handled;
            }

            if control_id == m.combo_box_email_id {
                fill_for_email(wm, h, &email);
            }
            // else if (controlID == comboBoxLoginNameID) {}
        }
        ggm::GCM_SELECTED => {
            if h.wol_login_menu().button_pushed {
                return MsgHandled::Handled;
            }
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            let m = h.wol_login_menu().clone();
            if control_id == m.combo_box_email_id {
                let u_email = m.combo_box_email.map(|c| cb::get_text(wm, h, c)).unwrap_or_default();
                let email = ascii(&u_email);
                fill_for_email(wm, h, &email);
            }
            // else if (controlID == comboBoxLoginNameID) {}
        }
        ggm::GBM_SELECTED => {
            if h.wol_login_menu().button_pushed {
                return MsgHandled::Handled;
            }
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            let m = h.wol_login_menu().clone();
            if control_id == m.button_back_id {
                h.wol_login_menu().button_pushed = true;
                h.tear_down_game_spy();
                shell::pop(wm, h);
            } else if control_id == m.button_create_account_id || control_id == m.button_login_id {
                let create = control_id == m.button_create_account_id;
                let mut month = ascii(&te::get_text(wm, h, m.text_entry_month));
                let mut day = ascii(&te::get_text(wm, h, m.text_entry_day));
                let year = ascii(&te::get_text(wm, h, m.text_entry_year));
                let today = h.today_year_month_day();
                if !is_age_okay(&mut month, &mut day, &year, today) {
                    let title = h.fetch_text(b"GUI:AgeFailedTitle");
                    let body = h.fetch_text(b"GUI:AgeFailed");
                    h.gs_message_box_ok(wm, &title, &body);
                    return MsgHandled::Handled;
                }

                let email = m.combo_box_email.map(|c| ascii(&cb::get_text(wm, h, c))).unwrap_or_default();
                let login = m.combo_box_login_name.map(|c| ascii(&cb::get_text(wm, h, c))).unwrap_or_default();
                let password = ascii(&te::get_text(wm, h, m.text_entry_password));
                if !email.is_empty() && !login.is_empty() && !password.is_empty() {
                    let now = h.time_get_time();
                    h.wol_login_menu().login_attempt_time = now;
                    h.buddy_request_login(create, &login, &email, &password);

                    let remember = m.check_box_remember_password.is_some_and(|c| gadget::check_box_is_checked(wm, c));
                    if remember {
                        let mut date = month.clone();
                        date.extend_from_slice(&day);
                        date.extend_from_slice(&year);
                        if let Some(p) = h.wol_login_menu().login_pref.as_mut() {
                            p.entries.insert(b"lastName".to_vec(), login.clone());
                            p.entries.insert(b"lastEmail".to_vec(), email.clone());
                            p.entries.insert(b"useProfiles".to_vec(), b"yes".to_vec());
                            p.add_login(&email, &login, &password, &date);
                        }
                    } else if !create {
                        // only the login button forgets an unchecked login
                        if let Some(p) = h.wol_login_menu().login_pref.as_mut() {
                            p.forget_login(&email);
                        }
                    }
                    enable_login_controls(wm, h, false);
                    h.start_pings();
                } else {
                    // the create button's box uses GUI:Error as its title, the login button's GUI:GSErrorTitle
                    let title: &[u8] = if create { b"GUI:Error" } else { b"GUI:GSErrorTitle" };
                    no_login_info_box(wm, h, title, email.is_empty(), login.is_empty(), password.is_empty());
                }
            } else if control_id == m.button_tos_id {
                if let Some(p) = m.parent_tos {
                    wm.win_hide(p, false);
                }
                h.wol_login_menu().use_web_browser_for_tos = false; // loginPref->getBool("UseTOSBrowser", TRUE) is commented out
                let use_browser = h.wol_login_menu().use_web_browser_for_tos;
                if use_browser && h.has_web_browser() {
                    h.web_browser_create_window(b"TermsOfService", m.listbox_tos);
                    h.wol_login_menu().web_browser_active = true;
                } else {
                    if let Some(l) = m.listbox_tos {
                        lb::reset(wm, h, l);
                    }
                    if let Some(lines) = h.tos_file_lines() {
                        let tos_color = 0xFFFF_FFFFu32 as i32; // GameMakeColor(255, 255, 255, 255)
                        for ascii_line in lines {
                            let mut uni_line = h.multi_byte_to_wide_char(&ascii_line);
                            // trailing white space off
                            while uni_line.last().is_some_and(|&c| c == ' ' as u16 || (c >= 9 && c <= 13)) {
                                uni_line.pop();
                            }
                            if let Some(l) = m.listbox_tos {
                                lb::add_entry_text(wm, h, l, &uni_line, tos_color, -1, 0, false);
                            }
                        }
                    }
                }
                enable_login_controls(wm, h, false);
                if let Some(b) = m.button_back {
                    wm.win_enable(b, false);
                }
            } else if control_id == m.button_tos_ok_id {
                enable_login_controls(wm, h, true);
                if let Some(p) = m.parent_tos {
                    wm.win_hide(p, true);
                }
                if h.wol_login_menu().use_web_browser_for_tos && h.has_web_browser() && m.listbox_tos.is_some() {
                    h.web_browser_close_window(m.listbox_tos);
                }
                h.option_prefs_set_saw_tos_and_write();
                h.wol_login_menu().web_browser_active = false;
                if let Some(b) = m.button_back {
                    wm.win_enable(b, true);
                }
            }
        }
        ggm::GEM_EDIT_DONE => {}
        // (the GEM_UPDATE_TEXT nick filter, `isNickOkay`, is commented out upstream)
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
