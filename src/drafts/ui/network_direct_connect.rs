//! UNTESTED DRAFT (#202, #203): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/NetworkDirectConnect.cpp
//!     (the file's statics, `PopulateRemoteIPComboBox`, `UpdateRemoteIPList`, `HostDirectConnectGame`,
//!      `JoinDirectConnectGame`, `NetworkDirectConnectInit/Shutdown/Update/Input/System`)
//!
//! `TheLAN` (LANAPI), `LANPreferences` / `OptionPreferences` (INI files), `IPEnumeration` and the `LANbuttonPushed` /
//! `LANisShuttingDown` globals (LANAPICallbacks.cpp) are the host's.

use crate::client::gadget_combo_box as cb;
use crate::client::gadget_text_entry as te;
use crate::client::shell::{self, ShellHost};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::client::window_transitions as wt;

/// `g_lanPlayerNameLength` (LANAPI.h).
pub const G_LAN_PLAYER_NAME_LENGTH: usize = 12;

/// The file's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NetworkDirectConnect {
    pub is_shutting_down: bool,
    pub button_pushed: bool,
    pub button_back_id: i32,
    pub button_host_id: i32,
    pub button_join_id: i32,
    pub edit_player_name_id: i32,
    pub combobox_remote_ip_id: i32,
    pub static_local_ip_id: i32,
    pub button_back: Option<WinId>,
    pub button_host: Option<WinId>,
    pub button_join: Option<WinId>,
    pub edit_player_name: Option<WinId>,
    pub combobox_remote_ip: Option<WinId>,
    pub static_local_ip: Option<WinId>,
}

/// What the direct connect screen reaches outside the window code.
pub trait NetworkDirectConnectHost: ShellHost {
    fn network_direct_connect(&mut self) -> &mut NetworkDirectConnect;
    /// `extern Bool LANbuttonPushed`, `LANisShuttingDown`.
    fn set_lan_button_pushed(&mut self, v: bool);
    fn set_lan_is_shutting_down(&mut self, v: bool);
    /// `TheGameText->fetch(label)`.
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    // LANPreferences (Network.ini)
    fn lan_prefs_get_user_name(&mut self) -> Vec<u16>;
    fn lan_prefs_get_num_remote_ips(&mut self) -> i32;
    fn lan_prefs_get_remote_ip_entry(&mut self, i: i32) -> Vec<u16>;
    /// `prefs[key] = value` (the pending writes, flushed by `write`).
    fn lan_prefs_set(&mut self, key: &[u8], value: &[u8]);
    fn lan_prefs_write(&mut self);
    /// `UnicodeStringToQuotedPrintable(name)`.
    fn unicode_to_quoted_printable(&self, name: &[u16]) -> Vec<u8>;
    // TheLAN
    fn lan_exists(&self) -> bool;
    /// `TheLAN = NEW LANAPI(); TheLAN->init()` (init only when `init`); `delete TheLAN; TheLAN = NULL`.
    fn lan_create(&mut self, init: bool);
    fn lan_delete(&mut self);
    fn lan_reset(&mut self);
    fn lan_get_local_ip(&self) -> u32;
    fn lan_set_local_ip(&mut self, ip: u32);
    fn lan_request_set_name(&mut self, name: &[u16]);
    fn lan_request_game_create(&mut self, ip_string: &[u16], direct_connect: bool);
    fn lan_request_game_join_direct_connect(&mut self, ip: u32);
    fn lan_request_lobby_leave(&mut self, forced: bool);
    /// `OptionPreferences::getOnlineIPAddress()`; the enumerated local addresses (`IPEnumeration::getAddresses()`).
    fn option_prefs_online_ip_address(&mut self) -> u32;
    fn enumerated_ip_addresses(&mut self) -> Vec<u32>;
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// `GameMakeColor(255,255,255,255)`.
const WHITE: i32 = 0xFFFF_FFFFu32 as i32;

/// `swscanf(s, L"%d.%d.%d.%d", ...)`: the fields read before the first mismatch.
fn scan_ip(s: &[u16]) -> Vec<i32> {
    let text: String = String::from_utf16_lossy(s);
    let mut out = Vec::new();
    let mut rest = text.trim_start();
    for n in 0..4 {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit() || (*c == '-' && false)).collect();
        if digits.is_empty() {
            break;
        }
        out.push(digits.parse::<i32>().unwrap_or(0));
        rest = &rest[digits.len()..];
        if n < 3 {
            if let Some(r) = rest.strip_prefix('.') {
                rest = r;
            } else {
                break;
            }
        }
    }
    out
}

/// `"%d.%d.%d.%d"` of a packed address.
fn ip_to_string(ip: u32) -> String {
    format!("{}.{}.{}.{}", ip >> 24, (ip & 0xff0000) >> 16, (ip & 0xff00) >> 8, ip & 0xff)
}

/// `PopulateRemoteIPComboBox()` (NetworkDirectConnect.cpp:86).
pub fn populate_remote_ip_combo_box<H: NetworkDirectConnectHost>(wm: &mut GameWindowManager, h: &mut H) {
    let combo = h.network_direct_connect().combobox_remote_ip;
    if let Some(c) = combo {
        cb::reset(wm, h, c);
    }
    let num_remote_ips = h.lan_prefs_get_num_remote_ips();
    for i in 0..num_remote_ips {
        let entry = h.lan_prefs_get_remote_ip_entry(i);
        if let Some(c) = combo {
            cb::add_entry(wm, h, c, &entry, WHITE);
        }
    }
    if num_remote_ips > 0 {
        if let Some(c) = combo {
            cb::set_selected_pos(wm, h, c, 0, true);
        }
    }
    h.lan_prefs_write();
}

/// `UpdateRemoteIPList()` (NetworkDirectConnect.cpp:113).
pub fn update_remote_ip_list<H: NetworkDirectConnectHost>(wm: &mut GameWindowManager, h: &mut H) {
    let Some(combo) = h.network_direct_connect().combobox_remote_ip else { return };
    let mut num_entries = cb::get_length(wm, combo);
    let current_selection = cb::get_selected_pos(wm, h, combo, -1);
    let unisel = cb::get_text(wm, h, combo);
    let sel: Vec<u8> = unisel.iter().map(|&c| c as u8).collect(); // sel.translate(unisel)
    // newEntry.nextToken(&newIP, ":")
    let new_ip: Vec<u16> = unisel.iter().copied().take_while(|&c| c != ':' as u16).collect();
    let n1 = scan_ip(&new_ip);
    if n1.len() != 4 {
        return;
    }

    h.lan_prefs_set(b"RemoteIP0", &sel);

    let mut current_ini_entry = 1;
    let mut i = 0;
    while i < num_entries {
        if i != current_selection {
            cb::set_selected_pos(wm, h, combo, i, false);
            let uni = cb::get_text(wm, h, combo);
            let ascii: Vec<u8> = uni.iter().map(|&c| c as u8).collect();

            if current_selection == -1 {
                let old_ip: Vec<u16> = uni.iter().copied().take_while(|&c| c != ':' as u16).collect();
                let n2 = scan_ip(&old_ip);
                // swscanf's result is not checked upstream; unread fields keep the stack's garbage — modelled as 0
                let is_equal = (0..4).all(|k| n1[k] == n2.get(k).copied().unwrap_or(0));
                if is_equal {
                    num_entries -= 1;
                    i += 1;
                    continue;
                }
            }

            let temp = format!("RemoteIP{current_ini_entry}");
            current_ini_entry += 1;
            h.lan_prefs_set(temp.as_bytes(), &ascii);
        }
        i += 1;
    }

    if current_selection == -1 {
        num_entries += 1;
    }
    h.lan_prefs_set(b"NumRemoteIPs", format!("{num_entries}").as_bytes());
    h.lan_prefs_write();
}

/// Stores `UserName` and trims `name` to `g_lanPlayerNameLength`, as the three places upstream do.
fn save_name_and_trim<H: NetworkDirectConnectHost>(wm: &mut GameWindowManager, h: &mut H, before_ip_list: Option<bool>) -> Vec<u16> {
    let edit = h.network_direct_connect().edit_player_name;
    let mut name = te::get_text(wm, h, edit);
    let qp = h.unicode_to_quoted_printable(&name);
    h.lan_prefs_set(b"UserName", &qp);
    h.lan_prefs_write();
    if before_ip_list == Some(true) {
        update_remote_ip_list(wm, h);
        populate_remote_ip_combo_box(wm, h);
    }
    while name.len() > G_LAN_PLAYER_NAME_LENGTH {
        name.pop();
    }
    name
}

/// `HostDirectConnectGame()` (NetworkDirectConnect.cpp:203).
pub fn host_direct_connect_game<H: NetworkDirectConnectHost>(wm: &mut GameWindowManager, h: &mut H) {
    if !h.lan_exists() {
        h.lan_create(false);
    }

    // get our IP
    let local_ip = h.lan_get_local_ip();
    let local_ip_string = utf16(&ip_to_string(local_ip));

    // get our name
    let name = save_name_and_trim(wm, h, None);

    h.lan_request_set_name(&name);
    h.lan_request_game_create(&local_ip_string, true);
}

/// `JoinDirectConnectGame()` (NetworkDirectConnect.cpp:245).
pub fn join_direct_connect_game<H: NetworkDirectConnectHost>(wm: &mut GameWindowManager, h: &mut H) {
    if !h.lan_exists() {
        h.lan_create(false);
    }

    let combo = h.network_direct_connect().combobox_remote_ip;
    let ipunistring = match combo {
        Some(c) => cb::get_text(wm, h, c),
        None => Vec::new(),
    };
    let asciientry: Vec<u8> = ipunistring.iter().map(|&c| c as u8).collect();
    // asciientry.nextToken(&ipstring, "(")
    let ipstring: Vec<u8> = asciientry.iter().copied().take_while(|&c| c != b'(').collect();
    // (`char ipstr[16]; strcpy(ipstr, ...)` overflows upstream for a longer string; not modelled)
    let ip_w: Vec<u16> = ipstring.iter().map(|&c| c as u16).collect();
    let f = scan_ip(&ip_w);
    let g = |k: usize| f.get(k).copied().unwrap_or(0);
    let ipaddress = ((g(0) << 24) + (g(1) << 16) + (g(2) << 8) + g(3)) as u32;

    // get our name
    let name = save_name_and_trim(wm, h, Some(true));

    h.lan_request_set_name(&name);
    h.lan_request_game_join_direct_connect(ipaddress);
}

/// `NetworkDirectConnectInit(layout, userData)` (NetworkDirectConnect.cpp:289).
pub fn network_direct_connect_init<H: NetworkDirectConnectHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    h.set_lan_button_pushed(false);
    h.set_lan_is_shutting_down(false);

    if !h.lan_exists() {
        h.lan_create(true);
    }
    h.lan_reset();

    h.network_direct_connect().button_pushed = false;
    h.network_direct_connect().is_shutting_down = false;

    shell::show_shell_map(wm, h, true);

    let key_of = |h: &mut H, n: &str| ShellHost::name_to_key(h, format!("NetworkDirectConnect.wnd:{n}").as_bytes()) as i32;
    let ids = [key_of(h, "ButtonBack"), key_of(h, "ButtonHost"), key_of(h, "ButtonJoin"), key_of(h, "EditPlayerName"), key_of(h, "ComboboxRemoteIP"), key_of(h, "StaticLocalIP")];
    let wins: Vec<Option<WinId>> = ids.iter().map(|&id| wm.win_get_window_from_id(None, id)).collect();
    {
        let d = h.network_direct_connect();
        d.button_back_id = ids[0];
        d.button_host_id = ids[1];
        d.button_join_id = ids[2];
        d.edit_player_name_id = ids[3];
        d.combobox_remote_ip_id = ids[4];
        d.static_local_ip_id = ids[5];
        d.button_back = wins[0];
        d.button_host = wins[1];
        d.button_join = wins[2];
        d.edit_player_name = wins[3];
        d.combobox_remote_ip = wins[4];
        d.static_local_ip = wins[5];
    }

    // Read in the user's name; use the default if it is blank
    let mut name = h.lan_prefs_get_user_name();
    if name.is_empty() {
        name = h.fetch_text(b"GUI:Player");
    }
    te::set_text(wm, h, wins[3], &name);

    populate_remote_ip_combo_box(wm, h);

    // `delete TheLAN; TheLAN = NULL;` and then, since it is NULL, make a new one
    h.lan_delete();
    {
        h.lan_create(false);
        let mut ip = h.option_prefs_online_ip_address();
        let addresses = h.enumerated_ip_addresses();
        // DEBUG_ASSERTCRASH(IPlist, ("No IP addresses found!")); upstream dereferences the list unchecked below
        let found_ip = addresses.iter().any(|&a| a == ip);
        if !found_ip {
            if let Some(&first) = addresses.first() {
                ip = first;
            }
        }
        h.lan_create(true); // TheLAN->init()
        h.lan_set_local_ip(ip);
    }
    let ip = h.lan_get_local_ip();
    let ipstr = utf16(&ip_to_string(ip));
    crate::client::gadget::static_text_set_text(wm, h, wins[5], &ipstr);

    // Clear the lobby of any games / players
    h.lan_request_lobby_leave(true);

    // Show Menu
    wm.layout_hide(layout, false);
    wm.layout_bring_forward(layout);
    wt::set_group(wm, h, b"NetworkDirectConnectFade", false);
}

/// `shutdownComplete(layout)`.
fn shutdown_complete<H: NetworkDirectConnectHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    h.network_direct_connect().is_shutting_down = false;

    // hide menu
    wm.layout_hide(layout, true);

    // our shutdown is complete
    shell::shutdown_complete(wm, h, Some(layout), false);
}

/// `NetworkDirectConnectShutdown(layout, userData)`; `pop_immediate` is `*(Bool *)userData`.
pub fn network_direct_connect_shutdown<H: NetworkDirectConnectHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize, pop_immediate: bool) {
    h.network_direct_connect().is_shutting_down = true;

    // if we are doing an immediate pop, just do it
    if pop_immediate {
        shutdown_complete(wm, h, layout);
        return;
    }

    shell::reverse_animate_window(wm, h);
    wt::reverse(wm, h, b"NetworkDirectConnectFade");
}

/// `NetworkDirectConnectUpdate(layout, userData)`.
pub fn network_direct_connect_update<H: NetworkDirectConnectHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    if h.network_direct_connect().is_shutting_down && shell::is_anim_finished(wm, h) && crate::client::window_transitions::is_finished(wm) {
        shutdown_complete(wm, h, layout);
    }
}

/// `NetworkDirectConnectInput(window, msg, mData1, mData2)`.
pub fn network_direct_connect_input<H: NetworkDirectConnectHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR {
        let k = (d1 & 0xFF) as u8;
        let state = d2 & 0xFF;
        // if we're showing the menu, ignore input
        if h.network_direct_connect().button_pushed {
            return MsgHandled::Ignored;
        }
        if k == key::ESC {
            // send a simulated selected event to the parent window of the back/exit button
            if state & key::STATE_UP != 0 {
                let d = h.network_direct_connect().clone();
                wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(d.button_back), d.button_back_id as u32);
            }
            // (end if) the escape is handled whatever the state
            return MsgHandled::Handled;
        }
    }
    MsgHandled::Ignored
}

/// `NetworkDirectConnectSystem(window, msg, mData1, mData2)`.
pub fn network_direct_connect_system<H: NetworkDirectConnectHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            // if we're givin the opportunity to take the keyboard focus we must say we want it
            if d1 == 1 {
                wm.focus_reply = true;
            }
            return MsgHandled::Handled;
        }
        ggm::GBM_SELECTED => {
            if h.network_direct_connect().button_pushed {
                return MsgHandled::Handled;
            }
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            let d = h.network_direct_connect().clone();
            if control_id == d.button_back_id {
                let name = save_name_and_trim(wm, h, None);
                h.lan_request_set_name(&name);

                h.network_direct_connect().button_pushed = true;
                h.set_lan_button_pushed(true);
                shell::pop(wm, h);
            } else if control_id == d.button_host_id {
                host_direct_connect_game(wm, h);
            } else if control_id == d.button_join_id {
                join_direct_connect_game(wm, h);
            }
        }
        ggm::GEM_EDIT_DONE => {}
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
