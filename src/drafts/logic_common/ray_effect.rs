//! UNTESTED DRAFT (#202, #208, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameClient/RayEffect.h
//!   GeneralsMD/Code/GameEngine/Source/GameClient/System/RayEffect.cpp
//!
//! "Ray effect system manager": a fixed table of 128 (drawable, start, end) entries that the beam draw modules read. A temporary
//! hack system "for the demo" upstream, kept as it is. The drawable pointer is the drawable's id here.

use crate::common::base_type::Coord3D;
use crate::common::game_type::ObjectId;

/// `MAX_RAY_EFFECTS`.
pub const MAX_RAY_EFFECTS: usize = 128;

/// `struct RayEffectData`: `draw` (`None` for NULL), `startLoc`, `endLoc`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayEffectData {
    /// `draw`: the drawable (by id).
    pub draw: Option<ObjectId>,
    /// `startLoc`.
    pub start_loc: Coord3D,
    /// `endLoc`.
    pub end_loc: Coord3D,
}

impl RayEffectData {
    fn zeroed() -> RayEffectData {
        RayEffectData { draw: None, start_loc: Coord3D { x: 0.0, y: 0.0, z: 0.0 }, end_loc: Coord3D { x: 0.0, y: 0.0, z: 0.0 } }
    }
}

/// `class RayEffectSystem`.
#[derive(Clone, Debug, PartialEq)]
pub struct RayEffectSystem {
    /// `m_effectData[MAX_RAY_EFFECTS]`.
    pub effect_data: Vec<RayEffectData>,
}

impl RayEffectSystem {
    /// `RayEffectSystem::RayEffectSystem()`: `init()`.
    pub fn new() -> RayEffectSystem {
        let mut s = RayEffectSystem { effect_data: Vec::new() };
        s.init();
        s
    }

    /// `RayEffectSystem::findEntry(draw)`: the first entry for this drawable.
    pub fn find_entry(&self, draw: ObjectId) -> Option<usize> {
        (0..MAX_RAY_EFFECTS).find(|&i| self.effect_data[i].draw == Some(draw))
    }

    /// `RayEffectSystem::init()`: every entry's `draw = NULL`, locations zeroed.
    pub fn init(&mut self) {
        self.effect_data = vec![RayEffectData::zeroed(); MAX_RAY_EFFECTS];
    }

    /// `RayEffectSystem::reset()`: "nothing dynamic going on here, just initialize it".
    pub fn reset(&mut self) {
        self.init();
    }

    /// `RayEffectSystem::addRayEffect(draw, startLoc, endLoc)`: ignores a NULL argument; takes the first free slot; no free slot, nothing.
    pub fn add_ray_effect(&mut self, draw: Option<ObjectId>, start_loc: Option<&Coord3D>, end_loc: Option<&Coord3D>) {
        // sanity
        let (draw, start_loc, end_loc) = match (draw, start_loc, end_loc) {
            (Some(d), Some(s), Some(e)) => (d, s, e),
            _ => return,
        };
        // search for a free effect slot
        let slot = (0..MAX_RAY_EFFECTS).find(|&i| self.effect_data[i].draw.is_none());
        // if no free slots we can't do it
        if let Some(i) = slot {
            // add the data to the entry
            self.effect_data[i].draw = Some(draw);
            self.effect_data[i].start_loc = *start_loc;
            self.effect_data[i].end_loc = *end_loc;
        }
    }

    /// `RayEffectSystem::deleteRayEffect(draw)`: frees the drawable's entry (`draw = NULL`; the locations stay as they were).
    pub fn delete_ray_effect(&mut self, draw: Option<ObjectId>) {
        let draw = match draw {
            Some(d) => d,
            None => return,
        };
        if let Some(i) = self.find_entry(draw) {
            self.effect_data[i].draw = None;
        }
    }

    /// `RayEffectSystem::getRayEffectData(draw, effectData)`: copies the entry into `effect_data` if found, else leaves it alone.
    pub fn get_ray_effect_data(&self, draw: Option<ObjectId>, effect_data: &mut RayEffectData) {
        let draw = match draw {
            Some(d) => d,
            None => return,
        };
        if let Some(i) = self.find_entry(draw) {
            *effect_data = self.effect_data[i];
        }
    }
}

impl Default for RayEffectSystem {
    fn default() -> Self {
        RayEffectSystem::new()
    }
}
