//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/Common/UserPreferences.cpp
//!     (`QuickMatchPreferences`, `CustomMatchPreferences`, `GameSpyMiscPreferences`)
//!
//! The verified `crate::client::user_preferences` translates the base class `UserPreferences` and leaves the online
//! subclasses out ("not translated" in its module docs). These are the subclasses' accessors, over the verified
//! `UserPreferences` (its `find`, `set_int`, `get_int` ...). What upstream reaches through globals is a host trait:
//! `TheGameSpyInfo->getLocalProfileID()`, `TheMultiplayerSettings`, `ThePlayerTemplateStore`, `TheChallengeGenerals`,
//! `TheGameInfo`, `GetRegistryLanguage()`, the map directory checks. The `stricmp` / `compareNoCase` tests are ASCII
//! case-insensitive on bytes.

use crate::client::user_preferences::UserPreferences;
use crate::common::map_cache::{ascii_string_to_quoted_printable, quoted_printable_to_ascii_string};

/// `PLAYERTEMPLATE_RANDOM`, `PLAYERTEMPLATE_OBSERVER`, `PLAYERTEMPLATE_MIN` (PlayerTemplate.h).
pub const PLAYERTEMPLATE_RANDOM: i32 = -1;
/// `PLAYERTEMPLATE_OBSERVER`.
pub const PLAYERTEMPLATE_OBSERVER: i32 = -2;
/// `PLAYERTEMPLATE_MIN`.
pub const PLAYERTEMPLATE_MIN: i32 = PLAYERTEMPLATE_OBSERVER;

/// The globals `CustomMatchPreferences` reads.
pub trait MatchPrefsHost {
    /// `TheGameSpyInfo->getLocalProfileID()`.
    fn local_profile_id(&self) -> i32;
    /// `TheMultiplayerSettings->getNumColors()`.
    fn num_colors(&self) -> i32;
    /// `ThePlayerTemplateStore->getPlayerTemplateCount()`.
    fn player_template_count(&self) -> i32;
    /// `getNthPlayerTemplate(n)` as `(starting building is empty, isOldFaction, name)`; `None` for NULL.
    fn nth_player_template(&self, n: i32) -> Option<(bool, bool, Vec<u8>)>;
    /// `TheGameInfo && TheGameInfo->oldFactionsOnly()`.
    fn old_factions_only(&self) -> bool;
    /// `TheChallengeGenerals->getGeneralByTemplateName(name)`: `Some(isStartingEnabled)`, `None` for NULL.
    fn general_starting_enabled(&self, template_name: &[u8]) -> Option<bool>;
    /// `GetRegistryLanguage()`.
    fn registry_language(&self) -> Vec<u8>;
    /// `TheMultiplayerSettings->getDefaultStartingMoney().countMoney()`.
    fn default_starting_money(&self) -> u32;
    /// `getDefaultOfficialMap()`.
    fn default_official_map(&self) -> Vec<u8>;
    /// `isValidMap(name, TRUE)`.
    fn is_valid_map(&self, name: &[u8]) -> bool;
    /// `isOfficialMap(name)`.
    fn is_official_map(&self, name: &[u8]) -> bool;
}

fn stricmp_eq(a: &[u8], b: &[u8]) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// `atoi` of a stored value (the verified `get_int` path handles the empty string with a default; upstream's `atoi` of
/// the found value is on the raw text, which `get_int` with a default of 0 reproduces for non-empty values, and an empty
/// stored value is 0 under `atoi` as well).
fn atoi_found(p: &mut UserPreferences, key: &[u8]) -> i32 {
    p.get_int(key, 0)
}

/// `QuickMatchPreferences` over the base `UserPreferences`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuickMatchPreferences {
    /// The base class.
    pub prefs: UserPreferences,
}

impl QuickMatchPreferences {
    /// `QuickMatchPreferences::QuickMatchPreferences()`: file name `GeneralsOnline\QMPref%d.ini`; the caller `load`s it.
    pub fn filename(local_profile: i32) -> Vec<u8> {
        format!("GeneralsOnline\\QMPref{}.ini", local_profile).into_bytes()
    }

    /// `setMapSelected(mapName, selected)`: key `AsciiStringToQuotedPrintable(mapName)`, "1" / "0".
    pub fn set_map_selected(&mut self, map_name: &[u8], selected: bool) {
        self.prefs.set_ascii_string(&ascii_string_to_quoted_printable(map_name), if selected { b"1" } else { b"0" });
    }

    /// `isMapSelected(mapName)`: TRUE when not found, else `atoi != 0`.
    pub fn is_map_selected(&mut self, map_name: &[u8]) -> bool {
        let key = ascii_string_to_quoted_printable(map_name);
        if self.prefs.find(&key).is_none() {
            return true;
        }
        atoi_found(&mut self.prefs, &key) != 0
    }

    /// `setLastLadder(addr, port)`: `LastLadderAddr`, `LastLadderPort` (`%d`).
    pub fn set_last_ladder(&mut self, addr: &[u8], port: u16) {
        self.prefs.set_ascii_string(b"LastLadderAddr", addr);
        self.prefs.set_ascii_string(b"LastLadderPort", port.to_string().as_bytes());
    }

    /// `getLastLadderAddr()`: empty when not found.
    pub fn get_last_ladder_addr(&self) -> Vec<u8> {
        self.prefs.find(b"LastLadderAddr").map(|v| v.to_vec()).unwrap_or_default()
    }

    /// `getLastLadderPort()`: 0 when not found; `atoi` truncated to `UnsignedShort`.
    pub fn get_last_ladder_port(&mut self) -> u16 {
        if self.prefs.find(b"LastLadderPort").is_none() {
            return 0;
        }
        atoi_found(&mut self.prefs, b"LastLadderPort") as u16
    }

    fn set_int_key(&mut self, key: &[u8], val: i32) {
        self.prefs.set_ascii_string(key, val.to_string().as_bytes());
    }

    fn get_int_key_or(&mut self, key: &[u8], default: i32) -> i32 {
        if self.prefs.find(key).is_none() {
            return default;
        }
        atoi_found(&mut self.prefs, key)
    }

    /// `setMaxDisconnects(val)`.
    pub fn set_max_disconnects(&mut self, val: i32) {
        self.set_int_key(b"MaxDisconnects", val);
    }
    /// `getMaxDisconnects()`: 0 when not found.
    pub fn get_max_disconnects(&mut self) -> i32 {
        self.get_int_key_or(b"MaxDisconnects", 0)
    }
    /// `setMaxPoints(val)`.
    pub fn set_max_points(&mut self, val: i32) {
        self.set_int_key(b"MaxPoints", val);
    }
    /// `getMaxPoints()`: 1000 when not found.
    pub fn get_max_points(&mut self) -> i32 {
        self.get_int_key_or(b"MaxPoints", 1000)
    }
    /// `setMinPoints(val)`.
    pub fn set_min_points(&mut self, val: i32) {
        self.set_int_key(b"MinPoints", val);
    }
    /// `getMinPoints()`: 0 when not found.
    pub fn get_min_points(&mut self) -> i32 {
        self.get_int_key_or(b"MinPoints", 0)
    }
    /// `setWaitTime(val)`.
    pub fn set_wait_time(&mut self, val: i32) {
        self.set_int_key(b"WaitTime", val);
    }
    /// `getWaitTime()`: 0 when not found.
    pub fn get_wait_time(&mut self) -> i32 {
        self.get_int_key_or(b"WaitTime", 0)
    }
    /// `setNumPlayers(val)`.
    pub fn set_num_players(&mut self, val: i32) {
        self.set_int_key(b"NumPlayers", val);
    }
    /// `getNumPlayers()`: 0 (first in list, 1v1) when not found.
    pub fn get_num_players(&mut self) -> i32 {
        self.get_int_key_or(b"NumPlayers", 0)
    }
    /// `setMaxPing(val)`.
    pub fn set_max_ping(&mut self, val: i32) {
        self.set_int_key(b"MaxPing", val);
    }
    /// `getMaxPing()`: 5 when not found.
    pub fn get_max_ping(&mut self) -> i32 {
        self.get_int_key_or(b"MaxPing", 5)
    }
    /// `setColor(val)`: `setInt("Color", val)`.
    pub fn set_color(&mut self, val: i32) {
        self.prefs.set_int(b"Color", val);
    }
    /// `getColor()`: `getInt("Color", 0)`.
    pub fn get_color(&mut self) -> i32 {
        self.prefs.get_int(b"Color", 0)
    }
    /// `setSide(val)`: `setInt("Side", val)`.
    pub fn set_side(&mut self, val: i32) {
        self.prefs.set_int(b"Side", val);
    }
    /// `getSide()`: `getInt("Side", 0)`.
    pub fn get_side(&mut self) -> i32 {
        self.prefs.get_int(b"Side", 0)
    }
}

/// `CustomMatchPreferences` over the base `UserPreferences`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomMatchPreferences {
    /// The base class.
    pub prefs: UserPreferences,
}

impl CustomMatchPreferences {
    /// `CustomMatchPreferences::CustomMatchPreferences()`: file name `GeneralsOnline\CustomPref%d.ini`.
    pub fn filename(local_profile: i32) -> Vec<u8> {
        format!("GeneralsOnline\\CustomPref{}.ini", local_profile).into_bytes()
    }

    fn set_int_key(&mut self, key: &[u8], val: i32) {
        self.prefs.set_ascii_string(key, val.to_string().as_bytes());
    }

    /// The `stricmp(it->second.str(), "1") == 0` pattern with a default when the key is missing.
    fn flag_or(&self, key: &[u8], default: bool) -> bool {
        match self.prefs.find(key) {
            None => default,
            Some(v) => stricmp_eq(v, b"1"),
        }
    }

    /// The `compareNoCase("yes") == 0` pattern with a default when the key is missing.
    fn yes_or(&self, key: &[u8], default: bool) -> bool {
        match self.prefs.find(key) {
            None => default,
            Some(v) => stricmp_eq(v, b"yes"),
        }
    }

    /// `setLastLadder(addr, port)`.
    pub fn set_last_ladder(&mut self, addr: &[u8], port: u16) {
        self.prefs.set_ascii_string(b"LastLadderAddr", addr);
        self.prefs.set_ascii_string(b"LastLadderPort", port.to_string().as_bytes());
    }

    /// `getLastLadderAddr()`: empty when not found.
    pub fn get_last_ladder_addr(&self) -> Vec<u8> {
        self.prefs.find(b"LastLadderAddr").map(|v| v.to_vec()).unwrap_or_default()
    }

    /// `getLastLadderPort()`: 0 when not found.
    pub fn get_last_ladder_port(&mut self) -> u16 {
        if self.prefs.find(b"LastLadderPort").is_none() {
            return 0;
        }
        self.prefs.get_int(b"LastLadderPort", 0) as u16
    }

    /// `getPreferredColor()`: -1 when not found or outside `[-1, numColors)`.
    pub fn get_preferred_color<H: MatchPrefsHost + ?Sized>(&mut self, host: &H) -> i32 {
        if self.prefs.find(b"Color").is_none() {
            return -1;
        }
        let ret = self.prefs.get_int(b"Color", 0);
        if ret < -1 || ret >= host.num_colors() {
            return -1;
        }
        ret
    }

    /// `setPreferredColor(val)`.
    pub fn set_preferred_color(&mut self, val: i32) {
        self.set_int_key(b"Color", val);
    }

    /// `getChatSizeSlider()`: 45 when not found or outside 0..=100.
    pub fn get_chat_size_slider(&mut self) -> i32 {
        if self.prefs.find(b"ChatSlider").is_none() {
            return 45;
        }
        let ret = self.prefs.get_int(b"ChatSlider", 0);
        if ret < 0 || ret > 100 {
            return 45;
        }
        ret
    }

    /// `setChatSizeSlider(val)`.
    pub fn set_chat_size_slider(&mut self, val: i32) {
        self.set_int_key(b"ChatSlider", val);
    }

    /// `getPreferredFaction()`.
    pub fn get_preferred_faction<H: MatchPrefsHost + ?Sized>(&mut self, host: &H) -> i32 {
        if self.prefs.find(b"PlayerTemplate").is_none() {
            return PLAYERTEMPLATE_RANDOM;
        }
        let mut ret = self.prefs.get_int(b"PlayerTemplate", 0);
        if ret == PLAYERTEMPLATE_OBSERVER || ret < PLAYERTEMPLATE_MIN || ret >= host.player_template_count() {
            ret = PLAYERTEMPLATE_RANDOM;
        }
        if ret >= 0 {
            match host.nth_player_template(ret) {
                None => ret = PLAYERTEMPLATE_RANDOM,
                Some((starting_building_empty, is_old_faction, name)) => {
                    if starting_building_empty {
                        ret = PLAYERTEMPLATE_RANDOM;
                    } else if host.old_factions_only() && !is_old_faction {
                        ret = PLAYERTEMPLATE_RANDOM;
                    } else {
                        // "Prevent from loading the disabled Generals" (disallowLockedGenerals is TRUE upstream).
                        let starts_locked = match host.general_starting_enabled(&name) {
                            Some(enabled) => !enabled,
                            None => false,
                        };
                        if starts_locked {
                            ret = PLAYERTEMPLATE_RANDOM;
                        }
                    }
                }
            }
        }
        ret
    }

    /// `setPreferredFaction(val)`.
    pub fn set_preferred_faction(&mut self, val: i32) {
        self.set_int_key(b"PlayerTemplate", val);
    }

    /// `usesSystemMapDir()`: TRUE when not found.
    pub fn uses_system_map_dir(&self) -> bool {
        self.flag_or(b"UseSystemMapDir", true)
    }
    /// `setUsesSystemMapDir(val)`.
    pub fn set_uses_system_map_dir(&mut self, val: bool) {
        self.set_int_key(b"UseSystemMapDir", val as i32);
    }

    /// `usesLongGameList()`: upstream `return TRUE;` first, the lookup after it is unreachable and kept out.
    pub fn uses_long_game_list(&self) -> bool {
        true
    }
    /// `setUsesLongGameList(val)`.
    pub fn set_uses_long_game_list(&mut self, val: bool) {
        self.set_int_key(b"UseLongGameList", val as i32);
    }

    /// `allowsObservers()`: TRUE when not found.
    pub fn allows_observers(&self) -> bool {
        self.flag_or(b"AllowObservers", true)
    }
    /// `setAllowsObserver(val)`.
    pub fn set_allows_observer(&mut self, val: bool) {
        self.set_int_key(b"AllowObservers", val as i32);
    }

    /// `getDisallowAsianText()`: when not found, FALSE for Chinese / Korean registry languages, else TRUE.
    pub fn get_disallow_asian_text<H: MatchPrefsHost + ?Sized>(&self, host: &H) -> bool {
        match self.prefs.find(b"DisallowAsianText") {
            None => {
                let lang = host.registry_language();
                !(stricmp_eq(&lang, b"chinese") || stricmp_eq(&lang, b"korean"))
            }
            Some(v) => stricmp_eq(v, b"1"),
        }
    }
    /// `setDisallowAsianText(val)`.
    pub fn set_disallow_asian_text(&mut self, val: bool) {
        self.set_int_key(b"DisallowAsianText", val as i32);
    }
    /// `getDisallowNonAsianText()`: FALSE when not found.
    pub fn get_disallow_non_asian_text(&self) -> bool {
        self.flag_or(b"DisallowNonAsianText", false)
    }
    /// `setDisallowNonAsianText(val)`.
    pub fn set_disallow_non_asian_text(&mut self, val: bool) {
        self.set_int_key(b"DisallowNonAsianText", val as i32);
    }

    /// `getPreferredMap()`.
    pub fn get_preferred_map<H: MatchPrefsHost + ?Sized>(&self, host: &H) -> Vec<u8> {
        let stored = match self.prefs.find(b"Map") {
            None => return host.default_official_map(),
            Some(v) => v.to_vec(),
        };
        let mut ret = quoted_printable_to_ascii_string(&stored);
        // `ret.trim()`: leading and trailing whitespace.
        while ret.last().map_or(false, |b| b.is_ascii_whitespace()) {
            ret.pop();
        }
        let lead = ret.iter().take_while(|b| b.is_ascii_whitespace()).count();
        ret.drain(..lead);
        if ret.is_empty() || !host.is_valid_map(&ret) {
            return host.default_official_map();
        }
        // "can only use official maps if recording stats"
        if self.get_use_stats() && !host.is_official_map(&ret) {
            ret = host.default_official_map();
        }
        ret
    }
    /// `setPreferredMap(val)`: stored quoted-printable under `Map`.
    pub fn set_preferred_map(&mut self, val: &[u8]) {
        self.prefs.set_ascii_string(b"Map", &ascii_string_to_quoted_printable(val));
    }

    /// `getSuperweaponRestricted()`: key `SuperweaponRestrict`, "yes", FALSE when not found.
    pub fn get_superweapon_restricted(&self) -> bool {
        self.yes_or(b"SuperweaponRestrict", false)
    }
    /// `setSuperweaponRestricted(v)`: "Yes" / "No".
    pub fn set_superweapon_restricted(&mut self, v: bool) {
        self.prefs.set_ascii_string(b"SuperweaponRestrict", if v { b"Yes" } else { b"No" });
    }

    /// `getStartingCash()`: the `Money`'s amount; default is the multiplayer settings' default starting money.
    /// `strtoul(.., NULL, 10)` of the stored text (leading digits, wraps at 32 bits on the platform's `unsigned long`).
    pub fn get_starting_cash<H: MatchPrefsHost + ?Sized>(&self, host: &H) -> u32 {
        match self.prefs.find(b"StartingCash") {
            None => host.default_starting_money(),
            Some(v) => {
                let mut i = 0;
                while i < v.len() && v[i].is_ascii_whitespace() {
                    i += 1;
                }
                let mut n: u32 = 0;
                while i < v.len() && v[i].is_ascii_digit() {
                    n = n.wrapping_mul(10).wrapping_add((v[i] - b'0') as u32);
                    i += 1;
                }
                n
            }
        }
    }
    /// `setStartingCash(startingCash)`: `%d` of `countMoney()`.
    pub fn set_starting_cash(&mut self, count_money: u32) {
        self.prefs.set_ascii_string(b"StartingCash", (count_money as i32).to_string().as_bytes());
    }

    /// `getFactionsLimited()`: key `LimitArmies`, FALSE when not found.
    pub fn get_factions_limited(&self) -> bool {
        self.yes_or(b"LimitArmies", false)
    }
    /// `setFactionsLimited(v)`.
    pub fn set_factions_limited(&mut self, v: bool) {
        self.prefs.set_ascii_string(b"LimitArmies", if v { b"Yes" } else { b"No" });
    }

    /// `getUseStats()`: key `UseStats`, TRUE when not found.
    pub fn get_use_stats(&self) -> bool {
        self.yes_or(b"UseStats", true)
    }
    /// `setUseStats(v)`.
    pub fn set_use_stats(&mut self, v: bool) {
        self.prefs.set_ascii_string(b"UseStats", if v { b"Yes" } else { b"No" });
    }
}

/// `GameSpyMiscPreferences` over the base `UserPreferences`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameSpyMiscPreferences {
    /// The base class.
    pub prefs: UserPreferences,
}

impl GameSpyMiscPreferences {
    /// `GameSpyMiscPreferences()`: file name `GeneralsOnline\GSMiscPref%d.ini`.
    pub fn filename(local_profile: i32) -> Vec<u8> {
        format!("GeneralsOnline\\GSMiscPref{}.ini", local_profile).into_bytes()
    }
    /// `getLocale()`.
    pub fn get_locale(&mut self) -> i32 {
        self.prefs.get_int(b"Locale", 0)
    }
    /// `setLocale(val)`.
    pub fn set_locale(&mut self, val: i32) {
        self.prefs.set_int(b"Locale", val);
    }
    /// `getCachedStats()`.
    pub fn get_cached_stats(&self) -> Vec<u8> {
        self.prefs.get_ascii_string(b"CachedStats", b"")
    }
    /// `setCachedStats(val)`.
    pub fn set_cached_stats(&mut self, val: &[u8]) {
        self.prefs.set_ascii_string(b"CachedStats", val);
    }
    /// `getQuickMatchResLocked()`.
    pub fn get_quick_match_res_locked(&mut self) -> bool {
        self.prefs.get_bool(b"QMResLock", false)
    }
    /// `getMaxMessagesPerUpdate()`.
    pub fn get_max_messages_per_update(&mut self) -> i32 {
        self.prefs.get_int(b"MaxMessagesPerUpdate", 100)
    }
}
