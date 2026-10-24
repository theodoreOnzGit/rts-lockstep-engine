// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/DefaultProductionExitUpdate.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/QueueProductionExitUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! What the production exit modules share, written once per file upstream: the create point and the natural rally point pushed through the
//! producer's transform (`getExitPosition`, `getNaturalRallyPoint`), and the rally point the player sets (`setRallyPoint`, `getRallyPoint`).

use crate::common::base_type::Coord3D;
use crate::common::module::ObjectId;
use crate::logic::weapon::PATHFIND_CELL_SIZE_F;
use crate::logic::world::game_logic::World;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// `transform->Transform_Vector(*transform, loc, &loc)` with the object's transform: `point` in the object's frame, in the world.
pub fn transform_point(world: &World, object: ObjectId, point: &Coord3D) -> Coord3D {
    let v = Vector3::new(point.x, point.y, point.z);
    let out = match world.objects.find(object) {
        Some(o) => Matrix3D::transform_vector(o.thing.transform_matrix(), &v),
        None => v,
    };
    Coord3D {
        x: out.x,
        y: out.y,
        z: out.z,
    }
}

/// `getExitPosition(exitPosition)`: the create point in the world (false only without an object).
pub fn exit_position(
    world: &World,
    object: ObjectId,
    unit_create_point: &Coord3D,
) -> Option<Coord3D> {
    world.objects.find(object)?;
    Some(transform_point(world, object, unit_create_point))
}

/// `getNaturalRallyPoint(rallyPoint, offset)`: the natural rally point, pushed out by two pathfinding cells along itself when `offset`, in the world.
pub fn natural_rally_point(
    world: &World,
    object: ObjectId,
    natural_rally_point: &Coord3D,
    offset: bool,
) -> Coord3D {
    let mut p = Vector3::new(
        natural_rally_point.x,
        natural_rally_point.y,
        natural_rally_point.z,
    );
    if offset {
        let mut off = p;
        off.normalize();
        off *= 2.0 * PATHFIND_CELL_SIZE_F;
        p += off;
    }
    transform_point(
        world,
        object,
        &Coord3D {
            x: p.x,
            y: p.y,
            z: p.z,
        },
    )
}

/// The rally point the player sets: `m_rallyPoint` and `m_rallyPointExists`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RallyPoint {
    /// `m_rallyPoint`: "Where units should move to after they have reached the "natural" rally point".
    pub point: Coord3D,
    /// `m_rallyPointExists`: "Only move to the rally point if this is true".
    pub exists: bool,
}

impl RallyPoint {
    /// `setRallyPoint(pos)`.
    pub fn set(&mut self, pos: &Coord3D) {
        self.point = *pos;
        self.exists = true;
    }

    /// `getRallyPoint()`: the point if there is one.
    pub fn get(&self) -> Option<Coord3D> {
        self.exists.then_some(self.point)
    }
}
