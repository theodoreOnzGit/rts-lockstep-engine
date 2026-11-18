//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameInfo.cpp (GameInfo::setSuperweaponRestriction)

use crate::common::game_info::GameInfo;

/// `GameInfo::setSuperweaponRestriction(restriction)` (GameInfo.cpp:702): `m_superweaponRestriction = restriction`. The
/// verified `GameInfo` has the field (`superweapon_restriction`, read by `game_info_to_ascii_string`) and no setter.
pub fn set_superweapon_restriction(game: &mut GameInfo, restriction: u16) {
    game.superweapon_restriction = restriction;
}
