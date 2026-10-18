// SPDX-License-Identifier: GPL-3.0-only
//! The identifiers of the world: what upstream's pointers between players,
//! teams and objects become.
//!
//! [`ObjectId`] and [`DrawableId`] are `common::game_type`'s (re-exported);
//! the two below are `Team.h`'s typedefs.

pub use crate::common::game_type::{DrawableId, ObjectId, INVALID_DRAWABLE_ID, INVALID_ID};

/// `typedef UnsignedInt TeamID` (`Team.h`): the id of a `Team` instance.
pub type TeamId = u32;
/// `TEAM_ID_INVALID`.
pub const TEAM_ID_INVALID: TeamId = 0;

/// `typedef UnsignedInt TeamPrototypeID`.
pub type TeamPrototypeId = u32;
/// `TEAM_PROTOTYPE_ID_INVALID`.
pub const TEAM_PROTOTYPE_ID_INVALID: TeamPrototypeId = 0;

/// A player's index in `PlayerList` (`Player::m_playerIndex`, 0 = neutral).
/// Upstream uses `Int` and `NameKeyType`; the index is the identity that
/// `Object`s, scripts and the network use.
pub type PlayerIndex = i32;
