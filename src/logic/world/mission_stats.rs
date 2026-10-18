// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/MissionStats.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/MissionStats.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `MissionStats`: the units and buildings killed and lost during a mission,
//! per owner where it matters. Upstream's header says what it is for (scoring
//! in multiplayer, and the AI's future moves) and that it is incomplete ("not
//! sure what need to be here").
//!
//! # What upstream does with it
//!
//! Only three things: the constructor and `init` zero it, and `xfer` saves
//! and loads it. No code in `GameEngine` writes its counters: they are
//! fields with no setter, so a counter stays zero until a later layer adds
//! the code that counts. The fields are public here for the same reason:
//! the record methods that will increment them belong to whoever counts the
//! events (layer 7's `Player` and the combat layers), and they will write these
//! fields directly, as upstream would.
//!
//! # Deliberate differences
//!
//! - `crc` is empty upstream and is not translated.
//! - The per-owner arrays are `[i32; MAX_PLAYER_COUNT]`, indexed by the owner's
//!   player index; an index outside `0..MAX_PLAYER_COUNT` is the caller's error
//!   (upstream indexes out of bounds).

use crate::common::game_common::MAX_PLAYER_COUNT;
use crate::common::xfer::{Xfer, XferResult, XferVersion};

/// `class MissionStats`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionStats {
    /// `m_unitsKilled[MAX_PLAYER_COUNT]`: units of each player we killed.
    pub units_killed: [i32; MAX_PLAYER_COUNT as usize],
    /// `m_unitsLost`: our units destroyed.
    pub units_lost: i32,
    /// `m_buildingsKilled[MAX_PLAYER_COUNT]`: buildings of each player we killed.
    pub buildings_killed: [i32; MAX_PLAYER_COUNT as usize],
    /// `m_buildingsLost`: our buildings destroyed.
    pub buildings_lost: i32,
}

impl Default for MissionStats {
    /// `MissionStats()`: `init()`.
    fn default() -> Self {
        MissionStats::new()
    }
}

impl MissionStats {
    /// `MissionStats::MissionStats()`.
    pub fn new() -> MissionStats {
        let mut m = MissionStats {
            units_killed: [0; MAX_PLAYER_COUNT as usize],
            units_lost: 0,
            buildings_killed: [0; MAX_PLAYER_COUNT as usize],
            buildings_lost: 0,
        };
        m.init();
        m
    }

    /// `init()`: every stat back to "nothing".
    pub fn init(&mut self) {
        self.units_killed = [0; MAX_PLAYER_COUNT as usize];
        self.buildings_killed = [0; MAX_PLAYER_COUNT as usize];
        self.units_lost = 0;
        self.buildings_lost = 0;
    }

    /// `MissionStats::xfer`, version 1: the killed-units array as raw bytes
    /// (`xferUser`, native `Int`s), the units lost, the killed-buildings array
    /// and the buildings lost.
    pub fn xfer<X: Xfer + ?Sized>(&mut self, xfer: &mut X) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        xfer_int_array(xfer, &mut self.units_killed)?;
        xfer.xfer_int(&mut self.units_lost)?;
        xfer_int_array(xfer, &mut self.buildings_killed)?;
        xfer.xfer_int(&mut self.buildings_lost)
    }
}

/// `xferUser(array, sizeof(Int) * MAX_PLAYER_COUNT)`: the raw bytes of the
/// array, native (little-endian) `Int`s.
fn xfer_int_array<X: Xfer + ?Sized>(xfer: &mut X, values: &mut [i32; MAX_PLAYER_COUNT as usize]) -> XferResult<()> {
    let mut bytes = [0u8; 4 * MAX_PLAYER_COUNT as usize];
    for (i, v) in values.iter().enumerate() {
        bytes[4 * i..4 * i + 4].copy_from_slice(&v.to_le_bytes());
    }
    xfer.xfer_user(&mut bytes)?;
    for (i, v) in values.iter_mut().enumerate() {
        *v = i32::from_le_bytes([bytes[4 * i], bytes[4 * i + 1], bytes[4 * i + 2], bytes[4 * i + 3]]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::xfer_load::XferLoad;
    use crate::common::xfer_save::XferSave;

    #[test]
    fn init_clears_every_counter() {
        let mut m = MissionStats::new();
        m.units_killed[15] = 4;
        m.buildings_killed[0] = 2;
        m.units_lost = 1;
        m.buildings_lost = 3;
        m.init();
        assert_eq!(m, MissionStats::new());
    }

    #[test]
    fn xfer_round_trips_the_counters() {
        let mut m = MissionStats::new();
        m.units_killed = std::array::from_fn(|i| i as i32 * 7 - 40);
        m.buildings_killed = std::array::from_fn(|i| -(i as i32));
        m.units_lost = -9;
        m.buildings_lost = 12;
        let mut save = XferSave::new();
        save.open(b"mission").expect("open");
        m.xfer(&mut save).expect("save");
        save.close().expect("close");
        let mut load = XferLoad::new(save.take_bytes());
        load.open(b"mission").expect("open");
        let mut back = MissionStats::new();
        back.xfer(&mut load).expect("load");
        assert_eq!(back, m);
    }
}
