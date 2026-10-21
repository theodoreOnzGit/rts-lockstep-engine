// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/Thing/Thing.cpp   (`Thing::isSignificantlyAboveTerrain`)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Questions a layer-9 module asks of its object by id that no other file of the world answers (`p1w11-sonnet`).

use crate::common::game_type::ObjectId;
use crate::logic::world::game_logic::World;

impl World {
    /// `obj->isAboveTerrain()`: `getHeightAboveTerrain() > 0`. `false` for an object that does not exist (its height is 0).
    pub fn is_above_terrain(&mut self, id: ObjectId) -> bool {
        self.get_height_above_terrain(id) > 0.0
    }

    /// `obj->isSignificantlyAboveTerrain()`: the height above the terrain (the layer the object is on) is more than `-(3*3)` times the gravity.
    /// `false` for an object that does not exist.
    pub fn is_significantly_above_terrain(&mut self, id: ObjectId) -> bool {
        if self.objects.find(id).is_none() {
            return false;
        }
        let threshold = (-(3 * 3)) as f32 * self.cuts.globals.gravity;
        self.get_height_above_terrain(id) > threshold
    }
}
