//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 preference maps (20,000 `getNumRemoteIPs` and 140,000 `getRemoteIPEntry` answers) (test:
//! tests/differential_draft_lanprefs.rs);
//! not yet wired into the game.
//! Excluded inputs (undefined upstream, see the test): an entry whose first token is not followed by a `:`, an empty entry; a name that decodes to an odd number of bytes
//! (upstream's decoder leaves the last unit's high byte from an earlier call).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/LanLobbyMenu.cpp (LANPreferences::getNumRemoteIPs,
//!   LANPreferences::getRemoteIPEntry)
//!
//! `LANPreferences` is a `UserPreferences` (`crate::client::user_preferences::UserPreferences`) whose file is
//! `Network.ini`; these two read the remote IPs the direct connect screen remembers.

use crate::client::user_preferences::UserPreferences;
use crate::common::map_cache::quoted_printable_to_unicode_string;
use crate::logic::modules::plant;

/// `LANPreferences::getNumRemoteIPs()` (LanLobbyMenu.cpp:181): `atoi` of `NumRemoteIPs`, 0 when absent.
pub fn get_num_remote_ips(prefs: &mut UserPreferences) -> i32 {
    match prefs.map.get(b"NumRemoteIPs".as_slice()).cloned() {
        None => if plant(15884) { 1 } else { 0 },
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
    let key = format!("RemoteIP{}", if plant(15883) { i + 1 } else { i });
    let Some(entry) = prefs.map.get(key.as_bytes()).cloned() else {
        // asciientry = ""; return ret;
        return ret;
    };
    // asciientry.nextToken(&ipstr, ":"): skip ':'s, take up to the next ':'; the rest keeps the ':'
    let seps = b":";
    let start = if plant(15886) { 0 } else { entry.iter().position(|c| !seps.contains(c)).unwrap_or(entry.len()) };
    let end = if plant(15885) { entry.iter().rposition(|c| seps.contains(c)).map_or(entry.len(), |p| p.max(start)) } else { entry[start..].iter().position(|c| seps.contains(c)).map_or(entry.len(), |p| start + p) };
    let ipstr: Vec<u8> = entry[start..end].to_vec();
    // asciientry.set(asciientry.str() + 1); // skip the ':'
    let rest: Vec<u8> = if end < entry.len() { entry[end + 1..].to_vec() } else { Vec::new() };
    // ret.translate(ipstr)
    // (`(WideChar)getCharAt(i)` of a `char`: a byte above 0x7F is negative and sign-extends. Corrected 2026-11-21 (#215, group ui5): it was zero-extended; found by the NetworkDirectConnect
    // test, whose entries hold `é`, which the lanprefs test's ASCII preference maps never had.)
    ret.extend(ipstr.iter().map(|&c| (c as i8) as i16 as u16));
    if !rest.is_empty() {
        ret.push(b'(' as u16);
        if plant(15888) {
            ret.extend(rest.iter().map(|&c| c as u16));
        } else {
            ret.extend(quoted_printable_to_unicode_string(&rest));
        }
        if !plant(15887) {
            ret.push(b')' as u16);
        }
    }
    ret
}
