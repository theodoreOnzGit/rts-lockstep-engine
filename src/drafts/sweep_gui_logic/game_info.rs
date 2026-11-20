//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on all 65,536 values (test: tests/differential_draft_glsmall.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameInfo.cpp (GameInfo::setSuperweaponRestriction)

use crate::common::game_info::GameInfo;
use crate::logic::modules::plant;

/// `GameInfo::setSuperweaponRestriction(restriction)` (GameInfo.cpp:702): `m_superweaponRestriction = restriction`. The
/// verified `GameInfo` has the field (`superweapon_restriction`, read by `game_info_to_ascii_string`) and no setter.
pub fn set_superweapon_restriction(game: &mut GameInfo, restriction: u16) {
    game.superweapon_restriction = if plant(15891) { restriction & 0xFF } else { restriction };
}
