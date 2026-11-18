//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/LanLobbyMenu.cpp (LANPreferences::getNumRemoteIPs,
//!   LANPreferences::getRemoteIPEntry)
//!
//! `LANPreferences` is a `UserPreferences` (`crate::client::user_preferences::UserPreferences`) whose file is
//! `Network.ini`; these two read the remote IPs the direct connect screen remembers.

use crate::client::user_preferences::UserPreferences;
use crate::common::map_cache::quoted_printable_to_unicode_string;

/// `LANPreferences::getNumRemoteIPs()` (LanLobbyMenu.cpp:181): `atoi` of `NumRemoteIPs`, 0 when absent.
pub fn get_num_remote_ips(prefs: &mut UserPreferences) -> i32 {
    match prefs.map.get(b"NumRemoteIPs".as_slice()).cloned() {
        None => 0,
        Some(v) => crate::client::user_preferences::atoi(&v, &mut prefs.undefined),
    }
}

/// `LANPreferences::getRemoteIPEntry(i)` (LanLobbyMenu.cpp:195): the entry `RemoteIP<i>` is `ip:name` (the name in
/// quoted-printable); the result is `ip` widened, then `(name)` when there is a name. An absent entry gives an empty
/// string (upstream returns the empty `ret`). An entry with no `:` makes upstream's `asciientry.set(str() + 1)` read one
/// past the end of the string; it is taken as an empty name here.
pub fn get_remote_ip_entry(prefs: &UserPreferences, i: i32) -> Vec<u16> {
    let mut ret: Vec<u16> = Vec::new();
    // key.format("RemoteIP%d", i)
    let key = format!("RemoteIP{}", i);
    let Some(entry) = prefs.map.get(key.as_bytes()).cloned() else {
        // asciientry = ""; return ret;
        return ret;
    };
    // asciientry.nextToken(&ipstr, ":"): skip ':'s, take up to the next ':'; the rest keeps the ':'
    let seps = b":";
    let start = entry.iter().position(|c| !seps.contains(c)).unwrap_or(entry.len());
    let end = entry[start..].iter().position(|c| seps.contains(c)).map_or(entry.len(), |p| start + p);
    let ipstr: Vec<u8> = entry[start..end].to_vec();
    // asciientry.set(asciientry.str() + 1); // skip the ':'
    let rest: Vec<u8> = if end < entry.len() { entry[end + 1..].to_vec() } else { Vec::new() };
    // ret.translate(ipstr)
    ret.extend(ipstr.iter().map(|&c| c as u16));
    if !rest.is_empty() {
        ret.push(b'(' as u16);
        ret.extend(quoted_printable_to_unicode_string(&rest));
        ret.push(b')' as u16);
    }
    ret
}
