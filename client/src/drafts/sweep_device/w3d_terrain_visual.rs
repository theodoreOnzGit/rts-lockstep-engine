//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DTerrainVisual.cpp
//!
//! Sweep 2 (#213). Drafted: `intersectTerrain`, `changeWaterHeight`, `addFactionBibDrawable` (and `addFactionBib`, which
//! shares its corner arithmetic), `removeFactionBibDrawable`, `removeFactionBib`, `removeBibHighlighting`.
//!
//! Excluded (listed in the #213 comment): `addSeismicSimulation`, `handleSeismicSimulations` and `updateSeismicSimulations`
//! (W3DTerrainVisual.cpp:374-532) are inside `#ifdef DO_SEISMIC_SIMULATIONS`, which no header or project defines
//! (`grep -rn "define DO_SEISMIC_SIMULATIONS"` over GeneralsMD/Code finds nothing), and `SeismicSimulationNode`/its filters
//! are not in the port. The seismic helpers on `WorldHeightMap` (`sweep_device::world_height_map`) are drafted for when it is.

use super::base_height_map::{BaseHeightMapRenderObjClass, WaterHeightQuery};
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::{DrawableId, ObjectId};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;

/// `GEOMETRY_BOX` is the only geometry type with distinct major and minor radii.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BibGeometryType {
    Sphere,
    Cylinder,
    Box,
}

/// What `addFactionBib*` reads from the building's template and geometry.
#[derive(Clone, Copy, Debug)]
pub struct FactionBibSource {
    /// `getTemplate()->getFactoryExitWidth()`.
    pub factory_exit_width: f32,
    /// `getTemplate()->getFactoryExtraBibWidth()`.
    pub factory_extra_bib_width: f32,
    /// `GeometryInfo::getMajorRadius()` / `getMinorRadius()` / `getGeomType()`.
    pub major_radius: f32,
    pub minor_radius: f32,
    pub geom_type: BibGeometryType,
}

/// The shared body of `addFactionBibDrawable` / `addFactionBib` (W3DTerrainVisual.cpp:980, 1013): the four bib corners in
/// world space, from a rectangle around the building's origin that is `sizeX + extraWidth` / `sizeY + extraWidth` half
/// extents and reaches `exitWidth` farther along +x (the factory exit), transformed by the building's matrix.
pub fn faction_bib_corners(transform: &Matrix3D, src: &FactionBibSource, extra: f32) -> [Vector3; 4] {
    let pos = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
    let exit_width = src.factory_exit_width;
    let extra_width = src.factory_extra_bib_width + extra;
    let size_x = src.major_radius;
    let mut size_y = src.minor_radius;
    if src.geom_type != BibGeometryType::Box {
        size_y = size_x;
    }
    let mut corners = [Vector3 { x: pos.x, y: pos.y, z: pos.z }; 4];
    corners[0].x -= size_x + extra_width;
    corners[0].y -= size_y + extra_width;
    corners[1].x += size_x + exit_width + extra_width;
    corners[1].y -= size_y + extra_width;
    corners[2].x += size_x + exit_width + extra_width;
    corners[2].y += size_y + extra_width;
    corners[3].x -= size_x + extra_width;
    corners[3].y += size_y + extra_width;
    for c in corners.iter_mut() {
        *c = Matrix3D::transform_vector(transform, c);
    }
    corners
}

/// `W3DTerrainVisual::addFactionBibDrawable` (W3DTerrainVisual.cpp:980). `have_height_map` is the `if (m_logicHeightMap)`
/// test (`DO_SEISMIC_SIMULATIONS` is undefined, so the logic map, not the client map).
pub fn add_faction_bib_drawable<W: WaterHeightQuery>(
    terrain: &mut BaseHeightMapRenderObjClass<W>,
    have_height_map: bool,
    transform: &Matrix3D,
    src: &FactionBibSource,
    drawable_id: DrawableId,
    highlight: bool,
    extra: f32,
) {
    if have_height_map {
        let corners = faction_bib_corners(transform, src, extra);
        terrain.add_terrain_bib_drawable(&corners, drawable_id, highlight);
    }
}

/// `W3DTerrainVisual::addFactionBib` (W3DTerrainVisual.cpp:1013).
pub fn add_faction_bib<W: WaterHeightQuery>(
    terrain: &mut BaseHeightMapRenderObjClass<W>,
    have_height_map: bool,
    transform: &Matrix3D,
    src: &FactionBibSource,
    object_id: ObjectId,
    highlight: bool,
    extra: f32,
) {
    if have_height_map {
        let corners = faction_bib_corners(transform, src, extra);
        terrain.add_terrain_bib(&corners, object_id, highlight);
    }
}

/// `W3DTerrainVisual::removeFactionBibDrawable` (W3DTerrainVisual.cpp:1064): only if the terrain render object exists.
pub fn remove_faction_bib_drawable<W: WaterHeightQuery>(terrain: Option<&mut BaseHeightMapRenderObjClass<W>>, id: DrawableId) {
    if let Some(t) = terrain {
        t.remove_terrain_bib_drawable(id);
    }
}

/// `W3DTerrainVisual::removeFactionBib` (W3DTerrainVisual.cpp:1073).
pub fn remove_faction_bib<W: WaterHeightQuery>(terrain: Option<&mut BaseHeightMapRenderObjClass<W>>, id: ObjectId) {
    if let Some(t) = terrain {
        t.remove_terrain_bib(id);
    }
}

/// `W3DTerrainVisual::removeBibHighlighting` (W3DTerrainVisual.cpp:1091).
pub fn remove_bib_highlighting<W: WaterHeightQuery>(terrain: Option<&mut BaseHeightMapRenderObjClass<W>>) {
    if let Some(t) = terrain {
        t.remove_terrain_bib_highlighting();
    }
}

/// `W3DTerrainVisual::changeWaterHeight` (W3DTerrainVisual.cpp:897): only if the water render object exists.
pub fn change_water_height(water: Option<&mut super::w3d_water::WaterRenderObjClass>, x: f32, y: f32, delta: f32) {
    if let Some(w) = water {
        w.change_grid_height(x, y, delta);
    }
}

/// `W3DTerrainVisual::intersectTerrain` (W3DTerrainVisual.cpp:734): the contact point of a segment with the terrain, or
/// None for a NULL ray end, no terrain object, or a miss. `cast_ray(start, end)` is `m_terrainRenderObject->Cast_Ray(
/// RayCollisionTestClass(LineSegClass(start, end)))` returning `Result->ContactPoint` on a hit (`Cast_Ray` is in
/// `BaseHeightMap.cpp`, outside this sweep's list).
pub fn intersect_terrain(
    ray_start: Option<&Coord3D>,
    ray_end: Option<&Coord3D>,
    have_terrain_render_object: bool,
    cast_ray: &impl Fn(Vector3, Vector3) -> Option<Vector3>,
) -> Option<Coord3D> {
    let (start, end) = match (ray_start, ray_end) {
        (Some(s), Some(e)) => (s, e),
        _ => return None,
    };
    if have_terrain_render_object {
        let hit = cast_ray(Vector3 { x: start.x, y: start.y, z: start.z }, Vector3 { x: end.x, y: end.y, z: end.z });
        if let Some(point) = hit {
            return Some(Coord3D { x: point.x, y: point.y, z: point.z });
        }
    }
    None
}
