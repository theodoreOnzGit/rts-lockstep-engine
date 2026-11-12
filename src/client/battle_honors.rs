// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/BattleHonors.h (the BATTLE_HONOR_* bits, BH_CHALLENGE_MASK_*,
//     MAX_BATTLE_HONOR_COLUMNS, MAX_BATTLE_HONOR_IMAGE_WIDTH, MAX_BATTLE_HONOR_IMAGE_HEIGHT)
//   GeneralsMD/Code/GameEngine/Include/Common/SkirmishBattleHonors.h (class SkirmishBattleHonors)
//   GeneralsMD/Code/GameEngine/Source/Common/SkirmishBattleHonors.cpp (the whole file: SkirmishBattleHonors::
//     SkirmishBattleHonors, ~SkirmishBattleHonors and every getter and setter)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/PopupPlayerInfo.cpp (BattleHonorTooltip,
//     rowsToSkip, ResetBattleHonorInsertion, InsertBattleHonor)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The skirmish battle honours (GitHub #92, #157): `SkirmishBattleHonors`, the player's skirmish record kept in
//! `SkirmishStats.ini`, and the honours list box's insertion and tooltip.
//!
//! ~~#92 calls this BattleHonors.cpp.~~ **Corrected 2026-11-12, #157:** the release has no BattleHonors.cpp:
//! `ResetBattleHonorInsertion`, `InsertBattleHonor` and `BattleHonorTooltip` are defined in PopupPlayerInfo.cpp
//! (lines 299-510) and declared in SkirmishBattleHonors.h; BattleHonors.h holds the bits.
//!
//! # As upstream reads it
//!
//! **`SkirmishBattleHonors`** is a `UserPreferences` whose constructor loads `SkirmishStats.ini` from the user data
//! folder ([`crate::client::user_preferences`]). Its keys: `Wins`, `Losses`, `WinStreak`, `BestWinStreak`,
//! `Challenge` (the challenge medals' mask), `SCUD`, `PPC`, `Nuke` (booleans, set once), `Honors` (`setHonors` ORs a
//! bit in), `<map>_<slot state>` (the endurance medal: the number of computer opponents beaten on that map at that
//! skill), `LastHouse` (the last general's side), `LoyalGames`, `USACampaign_<d>`, `CHINACampaign_<d>`,
//! `GLACampaign_<d>` and `ChallengeCampaign<n>_<d>` (1 when complete at difficulty `d`). The numbers read with
//! `getInt(key, 0)`, the campaign flags as `getInt != 0`. *Reset* is the map's own `clear()` and `write()`.
//!
//! **`InsertBattleHonor(list, image, enabled, itemData, row, column, text, extra)`** puts the honour's image into
//! the list box at (`row`, `column`), 40x41 scaled from 800x600 to the display (`Int` of the float), white when
//! enabled and grey (80, 80, 80) when not; the item data is the honour's bit (with `BATTLE_HONOR_NOT_GAINED` when not
//! enabled), and the row above it in the same column gets `extra` (the streak or the wins). The column then moves
//! on; past the list's last column it goes back to 0 and the row down by one and `rowsToSkip` (which is never more
//! than 0: the text rows that would raise it are commented out upstream).
//!
//! **`BattleHonorTooltip(window, instData, mouse)`**: the cell under the mouse; none, or an item data of 0: the
//! general `TOOLTIP:BattleHonors`. Else the honour's text by the first of its bits in upstream's order (the
//! `...Disabled` text for an honour not gained), the streak's and the domination's by the row above's number; drawn
//! 1.5 times the tooltip's width.
//!
//! # Upstream quirks that are kept
//!
//! - `getEnduranceMedal` and the campaign getters format their keys with `%d` of the difficulty or slot state: any
//!   value reads and writes its own key.
//! - `setHonors` ORs into whatever `Honors` holds: a value that is not a number reads as `atoi`'s.
//! - `BattleHonorTooltip`'s not-gained branch has no text for `BLITZ5` and `OFFICERSCLUB` (an honour shown greyed
//!   with only those bits gets no tooltip at all); its gained branch tests `BLITZ5` before `BLITZ10`.
//! - `InsertBattleHonor`'s `extra` goes into row `row - 1`: on row 0 that is row -1, which the list ignores.
//! - That `extra` overwrites the item data of the honour above it: in `populateSkirmishBattleHonors` the second and
//!   fourth rows of honours (the air wing and battle tank; the domination and ultimate) clear the item data of the
//!   first two honours of the row above (the China and GLA campaigns; the endurance and apocalypse), whose tooltip is
//!   then the general `TOOLTIP:BattleHonors` (found by the #157 test, kept).
//!
//! # Where this differs from upstream
//!
//! - The file is read and written through the host's user data folder ([`UserDataFiles`]: the client's folder on disk,
//!   or OPFS in the browser), as every preference file (#93).
//! - `rowsToSkip` (PopupPlayerInfo.cpp's static) is the caller's ([`BattleHonorInsertion`]).
//!
//! # Verification
//!
//! Compared with the C++ (tests/differential_battle_honors.rs, 2026-11-12): SkirmishBattleHonors.cpp verbatim on the
//! real `UserPreferences` and C stdio, PopupPlayerInfo.cpp's insertion and tooltip, `populateSkirmishBattleHonors` and
//! the score screen's skirmish block: 20,000 cases (random stats files and call sequences, honours lists of 1 to 6
//! columns at four display sizes, tooltips, finished skirmishes), 86,256 list cells, **0 mismatches**; planted bugs
//! 9511 .. 9528, 9537, 9538 all caught (one replaced: see the test).
//!
//! # Representation
//!
//! [`SkirmishBattleHonors`]: the `UserPreferences` it is.

use crate::client::gadget_list_box as lb;
use crate::client::main_menu::{MenuHost, DIFFICULTY_EASY, DIFFICULTY_HARD, DIFFICULTY_NORMAL};
use crate::client::user_preferences::{UserDataFiles, UserPreferences};
use crate::client::window_manager::{GameWindowManager, GuiHost};
use crate::client::window_script::WinId;
use crate::common::ini::game_make_color;
use crate::logic::modules::plant;

/// BattleHonors.h's bits.
pub const BATTLE_HONOR_LADDER_CHAMP: i32 = 0x0000_0001;
pub const BATTLE_HONOR_STREAK: i32 = 0x0000_0002;
pub const BATTLE_HONOR_LOYALTY_USA: i32 = 0x0000_0020;
pub const BATTLE_HONOR_LOYALTY_CHINA: i32 = 0x0000_0040;
pub const BATTLE_HONOR_BATTLE_TANK: i32 = 0x0000_0080;
pub const BATTLE_HONOR_AIR_WING: i32 = 0x0000_0100;
pub const BATTLE_HONOR_LOYALTY_GLA: i32 = 0x0000_0200;
pub const BATTLE_HONOR_ENDURANCE: i32 = 0x0000_0400;
pub const BATTLE_HONOR_CAMPAIGN_USA: i32 = 0x0000_0800;
pub const BATTLE_HONOR_CAMPAIGN_CHINA: i32 = 0x0000_1000;
pub const BATTLE_HONOR_CAMPAIGN_GLA: i32 = 0x0000_2000;
pub const BATTLE_HONOR_BLITZ5: i32 = 0x0000_4000;
pub const BATTLE_HONOR_BLITZ10: i32 = 0x0000_8000;
pub const BATTLE_HONOR_FAIR_PLAY: i32 = 0x0001_0000;
pub const BATTLE_HONOR_APOCALYPSE: i32 = 0x0002_0000;
pub const BATTLE_HONOR_OFFICERSCLUB: i32 = 0x0004_0000;
pub const BATTLE_HONOR_DOMINATION: i32 = 0x0008_0000;
pub const BATTLE_HONOR_CHALLENGE_MODE: i32 = 0x0010_0000;
pub const BATTLE_HONOR_ULTIMATE: i32 = 0x0020_0000;
pub const BATTLE_HONOR_GLOBAL_GENERAL: i32 = 0x0040_0000;
pub const BATTLE_HONOR_DOMINATION_ONLINE: i32 = 0x0080_0000;
pub const BATTLE_HONOR_STREAK_ONLINE: i32 = 0x0100_0000;
pub const BATTLE_HONOR_CHALLENGE: i32 = 0x0200_0000;
pub const BATTLE_HONOR_NOT_GAINED: i32 = 0x0800_0000;

/// `BH_CHALLENGE_MASK_1` .. `BH_CHALLENGE_MASK_7`.
pub const BH_CHALLENGE_MASK: [i32; 7] = [0x0001, 0x0002, 0x0004, 0x0008, 0x0010, 0x0020, 0x0040];

/// `MAX_BATTLE_HONOR_COLUMNS`, `MAX_BATTLE_HONOR_IMAGE_WIDTH`, `MAX_BATTLE_HONOR_IMAGE_HEIGHT`.
pub const MAX_BATTLE_HONOR_COLUMNS: i32 = 4;
pub const MAX_BATTLE_HONOR_IMAGE_WIDTH: i32 = 40;
pub const MAX_BATTLE_HONOR_IMAGE_HEIGHT: i32 = 41;

/// The stats file's name (`load("SkirmishStats.ini")`).
pub const SKIRMISH_STATS_FILE: &[u8] = b"SkirmishStats.ini";

/// `class SkirmishBattleHonors : public UserPreferences`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkirmishBattleHonors {
    pub prefs: UserPreferences,
}

/// `key.format("%s_%d", ...)` and its kin.
fn key2(a: &[u8], sep: &[u8], n: i32) -> Vec<u8> {
    let mut k = a.to_vec();
    k.extend_from_slice(sep);
    k.extend_from_slice(n.to_string().as_bytes());
    k
}

impl SkirmishBattleHonors {
    /// `SkirmishBattleHonors()`: `load("SkirmishStats.ini")`.
    pub fn new<F: UserDataFiles + ?Sized>(fs: &mut F) -> Self {
        let mut prefs = UserPreferences::new();
        prefs.load(fs, SKIRMISH_STATS_FILE);
        SkirmishBattleHonors { prefs }
    }

    /// `write()` (UserPreferences').
    pub fn write<F: UserDataFiles + ?Sized>(&self, fs: &mut F) -> bool {
        self.prefs.write(fs)
    }

    /// `clear()` (the map's: the file name is kept).
    pub fn clear(&mut self) {
        self.prefs.map.clear();
    }

    pub fn set_wins(&mut self, val: i32) {
        self.prefs.set_int(b"Wins", val);
    }
    pub fn get_wins(&mut self) -> i32 {
        self.prefs.get_int(b"Wins", 0)
    }
    pub fn set_losses(&mut self, val: i32) {
        self.prefs.set_int(b"Losses", val);
    }
    pub fn get_losses(&mut self) -> i32 {
        self.prefs.get_int(b"Losses", 0)
    }
    pub fn set_win_streak(&mut self, val: i32) {
        self.prefs.set_int(b"WinStreak", val);
    }
    pub fn get_win_streak(&mut self) -> i32 {
        self.prefs.get_int(b"WinStreak", 0)
    }
    pub fn set_best_win_streak(&mut self, val: i32) {
        self.prefs.set_int(b"BestWinStreak", val);
    }
    pub fn get_best_win_streak(&mut self) -> i32 {
        self.prefs.get_int(b"BestWinStreak", 0)
    }
    /// `setChallengeMedals(val)`.
    pub fn set_challenge_medals(&mut self, val: i32) {
        self.prefs.set_int(b"Challenge", val);
    }
    pub fn get_challenge_medals(&mut self) -> i32 {
        self.prefs.get_int(b"Challenge", 0)
    }
    /// `setBuiltSCUD()`.
    pub fn set_built_scud(&mut self) {
        self.prefs.set_bool(b"SCUD", true);
    }
    /// `builtSCUD()`.
    pub fn built_scud(&mut self) -> bool {
        self.prefs.get_bool(b"SCUD", false)
    }
    pub fn set_built_particle_cannon(&mut self) {
        self.prefs.set_bool(b"PPC", true);
    }
    pub fn built_particle_cannon(&mut self) -> bool {
        self.prefs.get_bool(b"PPC", false)
    }
    pub fn set_built_nuke(&mut self) {
        // (plant 9514, differential builds only: the nuke kept under the cannon's key)
        self.prefs.set_bool(if plant(9514) { b"PPC" } else { b"Nuke" }, true);
    }
    pub fn built_nuke(&mut self) -> bool {
        self.prefs.get_bool(b"Nuke", false)
    }
    /// `setHonors(which)`: OR'd into `Honors`.
    pub fn set_honors(&mut self, which: i32) {
        let honors = self.prefs.get_int(b"Honors", 0);
        // (plant 9512, differential builds only: the bit replaces the others)
        self.prefs.set_int(b"Honors", if plant(9512) { which } else { honors | which });
    }
    pub fn get_honors(&mut self) -> i32 {
        self.prefs.get_int(b"Honors", 0)
    }
    /// `setEnduranceMedal(mapName, difficulty, numAIs)`: `<map>_<difficulty>`.
    pub fn set_endurance_medal(&mut self, map_name: &[u8], difficulty: i32, num_ais: i32) {
        self.prefs.set_int(&key2(map_name, b"_", difficulty), num_ais);
    }
    pub fn get_endurance_medal(&mut self, map_name: &[u8], difficulty: i32) -> i32 {
        // (plant 9511, differential builds only: the key without its `_`)
        let key = key2(map_name, if plant(9511) { b"" } else { b"_" }, difficulty);
        self.prefs.get_int(&key, 0)
    }
    /// `setLastGeneral(val)`: `LastHouse`.
    pub fn set_last_general(&mut self, val: &[u8]) {
        self.prefs.set_ascii_string(b"LastHouse", val);
    }
    pub fn get_last_general(&self) -> Vec<u8> {
        self.prefs.get_ascii_string(b"LastHouse", b"")
    }
    pub fn set_num_games_loyal(&mut self, val: i32) {
        self.prefs.set_int(b"LoyalGames", val);
    }
    pub fn get_num_games_loyal(&mut self) -> i32 {
        self.prefs.get_int(b"LoyalGames", 0)
    }
    pub fn set_usa_campaign_complete(&mut self, difficulty: i32) {
        self.prefs.set_int(&key2(b"USACampaign", b"_", difficulty), 1);
    }
    pub fn get_usa_campaign_complete(&mut self, difficulty: i32) -> bool {
        self.prefs.get_int(&key2(b"USACampaign", b"_", difficulty), 0) != 0
    }
    pub fn set_china_campaign_complete(&mut self, difficulty: i32) {
        self.prefs.set_int(&key2(b"CHINACampaign", b"_", difficulty), 1);
    }
    pub fn get_china_campaign_complete(&mut self, difficulty: i32) -> bool {
        self.prefs.get_int(&key2(b"CHINACampaign", b"_", difficulty), 0) != 0
    }
    pub fn set_gla_campaign_complete(&mut self, difficulty: i32) {
        self.prefs.set_int(&key2(b"GLACampaign", b"_", difficulty), 1);
    }
    pub fn get_gla_campaign_complete(&mut self, difficulty: i32) -> bool {
        self.prefs.get_int(&key2(b"GLACampaign", b"_", difficulty), 0) != 0
    }
    /// `setChallengeCampaignComplete(challenge, difficulty)`: `ChallengeCampaign<n>_<d>`.
    pub fn set_challenge_campaign_complete(&mut self, challenge: i32, difficulty: i32) {
        self.prefs.set_int(&key2(&key2(b"ChallengeCampaign", b"", challenge), b"_", difficulty), 1);
    }
    pub fn get_challenge_campaign_complete(&mut self, challenge: i32, difficulty: i32) -> bool {
        // (plant 9513, differential builds only: the challenge and the difficulty swapped in the key)
        let (a, b) = if plant(9513) { (difficulty, challenge) } else { (challenge, difficulty) };
        self.prefs.get_int(&key2(&key2(b"ChallengeCampaign", b"", a), b"_", b), 0) != 0
    }
}

/// The three difficulties the honours list tests, hardest first (`DIFFICULTY_HARD`, `NORMAL`, `EASY`).
pub const DIFFICULTIES_HARD_FIRST: [i32; 3] = [DIFFICULTY_HARD, DIFFICULTY_NORMAL, DIFFICULTY_EASY];

/// PopupPlayerInfo.cpp's `static Int rowsToSkip`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BattleHonorInsertion {
    pub rows_to_skip: i32,
}

/// `ResetBattleHonorInsertion()`.
pub fn reset_battle_honor_insertion(ins: &mut BattleHonorInsertion) {
    ins.rows_to_skip = 0;
}

/// `InsertBattleHonor(list, image, enabled, itemData, row, column, text, extra)` (the text is unused upstream: its
/// rows are commented out).
#[allow(clippy::too_many_arguments)]
pub fn insert_battle_honor<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, ins: &mut BattleHonorInsertion, list: WinId, image: Option<Vec<u8>>, enabled: bool, mut item_data: i32, row: &mut i32, column: &mut i32, extra: i32) {
    let (dw, dh) = wm.display;
    // `Int width = MAX_BATTLE_HONOR_IMAGE_WIDTH * (TheDisplay->getWidth() / 800.0f)` (UnsignedInt / float, truncated)
    let width = (MAX_BATTLE_HONOR_IMAGE_WIDTH as f32 * (dw as u32 as f32 / 800.0)) as i32;
    let height = (MAX_BATTLE_HONOR_IMAGE_HEIGHT as f32 * (dh as u32 as f32 / 600.0)) as i32;
    let enabled_color = 0xFFFF_FFFFu32 as i32;
    let disabled_color = game_make_color(80, 80, 80, 255);
    let color = if enabled { enabled_color } else { disabled_color };
    // (plant 9515, differential builds only: a greyed honour without its not-gained bit)
    if !enabled && !plant(9515) {
        item_data |= BATTLE_HONOR_NOT_GAINED;
    }
    lb::add_entry_image(wm, h, list, image, *row, *column, height, width, true, color);
    lb::set_item_data(wm, h, list, item_data as u32, *row, *column);
    // (plant 9517, differential builds only: the extra value on the honour's own row)
    let extra_row = if plant(9517) { *row } else { *row - 1 };
    lb::set_item_data(wm, h, list, extra as u32, extra_row, *column);
    *column += 1;
    // (plant 9516, differential builds only: the row ends one column late)
    let columns = lb::get_num_columns(wm, list);
    if if plant(9516) { *column > columns } else { *column >= columns } {
        *column = 0;
        *row = *row + 1 + ins.rows_to_skip;
        ins.rows_to_skip = (ins.rows_to_skip - 1).max(0);
    }
}

/// `BattleHonorTooltip`'s choice for a cell's item data (`battle_honor`) and the item data of the row above
/// (`extra_value`): the label, and whether it is drawn at 1.5 times the width (every honour's; the general text is at
/// the default). `None`: no tooltip is set (an honour whose bits have no text).
pub fn battle_honor_tooltip_label(battle_honor: i32, extra_value: i32) -> Option<(&'static [u8], bool)> {
    let bit = |b: i32| battle_honor & b != 0;
    if battle_honor == 0 {
        return Some((b"TOOLTIP:BattleHonors", false));
    }
    let l: &'static [u8] = if bit(BATTLE_HONOR_NOT_GAINED) {
        // (plant 9521, differential builds only: the battle tank tested before the GLA loyalty)
        if plant(9521) && bit(BATTLE_HONOR_BATTLE_TANK) {
            b"TOOLTIP:BattleHonorBattleTankDisabled"
        } else if bit(BATTLE_HONOR_LOYALTY_USA) {
            b"TOOLTIP:BattleHonorLoyaltyUSADisabled"
        } else if bit(BATTLE_HONOR_LOYALTY_CHINA) {
            b"TOOLTIP:BattleHonorLoyaltyChinaDisabled"
        } else if bit(BATTLE_HONOR_LOYALTY_GLA) {
            b"TOOLTIP:BattleHonorLoyaltyGLADisabled"
        } else if bit(BATTLE_HONOR_BATTLE_TANK) {
            b"TOOLTIP:BattleHonorBattleTankDisabled"
        } else if bit(BATTLE_HONOR_AIR_WING) {
            b"TOOLTIP:BattleHonorAirWingDisabled"
        } else if bit(BATTLE_HONOR_ENDURANCE) {
            b"TOOLTIP:BattleHonorEnduranceDisabled"
        } else if bit(BATTLE_HONOR_CAMPAIGN_USA) {
            b"TOOLTIP:BattleHonorCampaignUSADisabled"
        } else if bit(BATTLE_HONOR_CAMPAIGN_CHINA) {
            b"TOOLTIP:BattleHonorCampaignChinaDisabled"
        } else if bit(BATTLE_HONOR_CAMPAIGN_GLA) {
            b"TOOLTIP:BattleHonorCampaignGLADisabled"
        } else if bit(BATTLE_HONOR_BLITZ10) {
            b"TOOLTIP:BattleHonorBlitzDisabled"
        } else if bit(BATTLE_HONOR_FAIR_PLAY) {
            b"TOOLTIP:BattleHonorFairPlayDisabled"
        } else if bit(BATTLE_HONOR_APOCALYPSE) {
            b"TOOLTIP:BattleHonorApocalypseDisabled"
        } else if bit(BATTLE_HONOR_CHALLENGE_MODE) {
            b"TOOLTIP:BattleHonorCampaignChallengeDisabled"
        } else if bit(BATTLE_HONOR_ULTIMATE) {
            b"TOOLTIP:BattleHonorUltimateDisabled"
        } else if bit(BATTLE_HONOR_GLOBAL_GENERAL) {
            b"TOOLTIP:BattleHonorGlobalGeneralDisabled"
        } else if bit(BATTLE_HONOR_CHALLENGE) {
            b"TOOLTIP:BattleHonorChallengeDisabled"
        } else if bit(BATTLE_HONOR_STREAK) {
            b"TOOLTIP:BattleHonorStreakDisabled"
        } else if bit(BATTLE_HONOR_STREAK_ONLINE) {
            b"TOOLTIP:BattleHonorStreakOnlineDisabled"
        } else if bit(BATTLE_HONOR_DOMINATION) {
            b"TOOLTIP:BattleHonorDominationDisabled"
        } else if bit(BATTLE_HONOR_DOMINATION_ONLINE) {
            b"TOOLTIP:BattleHonorDominationOnlineDisabled"
        } else {
            return None;
        }
    } else if bit(BATTLE_HONOR_LOYALTY_USA) {
        b"TOOLTIP:BattleHonorLoyaltyUSA"
    } else if bit(BATTLE_HONOR_LOYALTY_CHINA) {
        b"TOOLTIP:BattleHonorLoyaltyChina"
    } else if bit(BATTLE_HONOR_LOYALTY_GLA) {
        b"TOOLTIP:BattleHonorLoyaltyGLA"
    } else if bit(BATTLE_HONOR_BATTLE_TANK) {
        b"TOOLTIP:BattleHonorBattleTank"
    } else if bit(BATTLE_HONOR_AIR_WING) {
        b"TOOLTIP:BattleHonorAirWing"
    } else if bit(BATTLE_HONOR_ENDURANCE) {
        b"TOOLTIP:BattleHonorEndurance"
    } else if bit(BATTLE_HONOR_CAMPAIGN_USA) {
        b"TOOLTIP:BattleHonorCampaignUSA"
    } else if bit(BATTLE_HONOR_CAMPAIGN_CHINA) {
        b"TOOLTIP:BattleHonorCampaignChina"
    } else if bit(BATTLE_HONOR_CAMPAIGN_GLA) {
        b"TOOLTIP:BattleHonorCampaignGLA"
    } else if bit(BATTLE_HONOR_BLITZ5) {
        b"TOOLTIP:BattleHonorBlitz5"
    } else if bit(BATTLE_HONOR_BLITZ10) {
        b"TOOLTIP:BattleHonorBlitz10"
    } else if bit(BATTLE_HONOR_FAIR_PLAY) {
        b"TOOLTIP:BattleHonorFairPlay"
    } else if bit(BATTLE_HONOR_APOCALYPSE) {
        b"TOOLTIP:BattleHonorApocalypse"
    } else if bit(BATTLE_HONOR_OFFICERSCLUB) {
        b"TOOLTIP:BattleHonorOfficersClub"
    } else if bit(BATTLE_HONOR_CHALLENGE_MODE) {
        b"TOOLTIP:BattleHonorCampaignChallenge"
    } else if bit(BATTLE_HONOR_ULTIMATE) {
        b"TOOLTIP:BattleHonorUltimate"
    } else if bit(BATTLE_HONOR_GLOBAL_GENERAL) {
        b"TOOLTIP:BattleHonorGlobalGeneral"
    } else if bit(BATTLE_HONOR_CHALLENGE) {
        b"TOOLTIP:BattleHonorChallenge"
    } else if bit(BATTLE_HONOR_STREAK) {
        // (plant 9522, differential builds only: the 25 streak from 24)
        let t25 = if plant(9522) { 24 } else { 25 };
        if extra_value >= 1000 {
            b"TOOLTIP:BattleHonorStreak1000"
        } else if extra_value >= 500 {
            b"TOOLTIP:BattleHonorStreak500"
        } else if extra_value >= 100 {
            b"TOOLTIP:BattleHonorStreak100"
        } else if extra_value >= t25 {
            b"TOOLTIP:BattleHonorStreak25"
        } else if extra_value >= 10 {
            b"TOOLTIP:BattleHonorStreak10"
        } else if extra_value >= 3 {
            b"TOOLTIP:BattleHonorStreak3"
        } else {
            b"TOOLTIP:BattleHonorStreakDisabled"
        }
    } else if bit(BATTLE_HONOR_STREAK_ONLINE) {
        if extra_value >= 1000 {
            b"TOOLTIP:BattleHonorStreak1000Online"
        } else if extra_value >= 500 {
            b"TOOLTIP:BattleHonorStreak500Online"
        } else if extra_value >= 100 {
            b"TOOLTIP:BattleHonorStreak100Online"
        } else if extra_value >= 25 {
            b"TOOLTIP:BattleHonorStreak25Online"
        } else if extra_value >= 10 {
            b"TOOLTIP:BattleHonorStreak10Online"
        } else if extra_value >= 3 {
            b"TOOLTIP:BattleHonorStreak3Online"
        } else {
            b"TOOLTIP:BattleHonorStreakOnlineDisabled"
        }
    } else if bit(BATTLE_HONOR_DOMINATION) {
        if extra_value >= 10000 {
            b"TOOLTIP:BattleHonorDomination10000"
        } else if extra_value >= 1000 {
            b"TOOLTIP:BattleHonorDomination1000"
        } else if extra_value >= 500 {
            b"TOOLTIP:BattleHonorDomination500"
        } else if extra_value >= 100 {
            b"TOOLTIP:BattleHonorDomination100"
        } else {
            b"TOOLTIP:BattleHonorDominationDisabled"
        }
    } else if bit(BATTLE_HONOR_DOMINATION_ONLINE) {
        if extra_value >= 10000 {
            b"TOOLTIP:BattleHonorDomination10000Online"
        } else if extra_value >= 1000 {
            b"TOOLTIP:BattleHonorDomination1000Online"
        } else if extra_value >= 500 {
            b"TOOLTIP:BattleHonorDomination500Online"
        } else if extra_value >= 100 {
            b"TOOLTIP:BattleHonorDomination100Online"
        } else {
            b"TOOLTIP:BattleHonorDominationOnlineDisabled"
        }
    } else {
        return None;
    };
    Some((l, true))
}

/// `BattleHonorTooltip(window, instData, mouse)`: the tooltip of the honour under the mouse (see the module docs).
pub fn battle_honor_tooltip<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, mouse: u32) {
    // (`LOLONGTOSHORT(mouse)`, `HILONGTOSHORT(mouse)`: masked, not sign-extended)
    let x = (mouse & 0x0000_FFFF) as i32;
    let y = ((mouse & 0xFFFF_0000) >> 16) as i32;
    let (row, col) = lb::get_entry_based_on_xy(wm, h, window, x, y).unwrap_or((-1, -1));
    let pick = if row == -1 || col == -1 {
        Some((&b"TOOLTIP:BattleHonors"[..], false))
    } else {
        let battle_honor = lb::get_item_data(wm, h, window, row, col) as i32;
        let extra_value = lb::get_item_data(wm, h, window, row - 1, col) as i32;
        battle_honor_tooltip_label(battle_honor, extra_value)
    };
    if let Some((label, wide)) = pick {
        let t = h.fetch_text(label);
        // (`setCursorTooltip(text)`: delay -1, width 1; an honour's `(text, -1, NULL, 1.5f)`)
        wm.mouse.set_cursor_tooltip_with(&t, -1, None, if wide { 1.5 } else { 1.0 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::user_preferences::MemoryUserData;

    /// The stats file's keys as SkirmishBattleHonors.cpp names them, read back after a write.
    /// Predicted: `Honors = 3` after two `setHonors`; the endurance key `maps\a\a.map_4`; the challenge key
    /// `ChallengeCampaign3_2`; `LastHouse = GLA`; the written file in key order with CR LF.
    #[test]
    fn the_stats_file_keeps_upstreams_keys() {
        let mut fs = MemoryUserData { path: b"U\\".to_vec(), ..Default::default() };
        let mut s = SkirmishBattleHonors::new(&mut fs);
        s.set_honors(BATTLE_HONOR_LADDER_CHAMP);
        s.set_honors(BATTLE_HONOR_STREAK);
        s.set_endurance_medal(b"maps\\a\\a.map", 4, 3);
        s.set_challenge_campaign_complete(3, DIFFICULTY_HARD);
        s.set_last_general(b"GLA");
        s.set_built_nuke();
        s.set_wins(7);
        assert!(s.write(&mut fs));
        let text = fs.files.get(&b"U\\SkirmishStats.ini"[..]).expect("written").clone();
        assert_eq!(String::from_utf8(text).expect("text"), "ChallengeCampaign3_2 = 1\r\nHonors = 3\r\nLastHouse = GLA\r\nNuke = 1\r\nWins = 7\r\nmaps\\a\\a.map_4 = 3\r\n");
        let mut r = SkirmishBattleHonors::new(&mut fs);
        assert_eq!((r.get_honors(), r.get_endurance_medal(b"maps\\a\\a.map", 4), r.get_wins(), r.get_last_general()), (3, 3, 7, b"GLA".to_vec()));
        assert!(r.get_challenge_campaign_complete(3, DIFFICULTY_HARD) && !r.get_challenge_campaign_complete(2, DIFFICULTY_HARD));
        assert!(r.built_nuke() && !r.built_scud());
        r.clear();
        assert_eq!(r.get_wins(), 0);
    }

    /// `BattleHonorTooltip`'s choice. Predicted: 0 the general text; a greyed streak `...StreakDisabled` whatever the
    /// streak; a streak of 25 `Streak25`; a greyed Blitz 5 or Officers' Club: none.
    #[test]
    fn the_tooltip_picks_as_upstream() {
        assert_eq!(battle_honor_tooltip_label(0, 0), Some((&b"TOOLTIP:BattleHonors"[..], false)));
        assert_eq!(battle_honor_tooltip_label(BATTLE_HONOR_STREAK | BATTLE_HONOR_NOT_GAINED, 99).map(|l| l.0), Some(&b"TOOLTIP:BattleHonorStreakDisabled"[..]));
        assert_eq!(battle_honor_tooltip_label(BATTLE_HONOR_STREAK, 25).map(|l| l.0), Some(&b"TOOLTIP:BattleHonorStreak25"[..]));
        assert_eq!(battle_honor_tooltip_label(BATTLE_HONOR_BLITZ5 | BATTLE_HONOR_NOT_GAINED, 0), None);
        assert_eq!(battle_honor_tooltip_label(BATTLE_HONOR_OFFICERSCLUB | BATTLE_HONOR_NOT_GAINED, 0), None);
    }
}
