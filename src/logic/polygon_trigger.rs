// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/PolygonTrigger.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Map/PolygonTrigger.cpp
//   GeneralsMD/Code/GameEngine/Include/Common/MapReaderWriterInfo.h
//     (the K_TRIGGERS_VERSION_* numbers)
//   GeneralsMD/Code/GameEngine/Include/Common/MapObject.h (MAP_XY_FACTOR)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: John Ahlquist, November 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `PolygonTrigger`: a trigger area, a polygon of integer points, used by
//! the scripts (a unit enters or leaves it) and as a water area or river of
//! the map. Its job here: the point-in-polygon test, the bounds and radius
//! that gate it, and the `PolygonTriggers` map chunk, read and written.
//!
//! # What upstream keeps in statics, and where it lives here
//!
//! `ThePolygonTriggerListPtr` (the head of the chain linked by
//! `m_nextPolygonTrigger`) and `s_currentID` (the next trigger id) are the
//! fields of [`PolygonTriggerList`]. The chain is a `Vec` in chain order
//! (the head first): `getNext()` is the next element. The parse fills the
//! list the way upstream does, and [`TriggerParse`] carries the list through
//! the chunk parser, which is generic over its context (a `fn` pointer, so
//! no trait object).
//!
//! # Bounds and radius
//!
//! Upstream caches the bounding box and the radius in `mutable` fields,
//! refreshed by `updateBounds` when a point was added. Here they are computed
//! from the points by each query that needs them. The values are the same:
//! upstream's constructor leaves the fields unset, but the memory it hands
//! out is zero-filled (`GameMemory.cpp`, the shim does the same), so a trigger
//! with no points reads the zero box and radius 0, and that is what
//! [`PolygonTrigger::bounds`] returns for it. The cost is one pass over the
//! points per query, where upstream does that pass once per change.
//!
//! # Undefined upstream, and what Rust does instead
//!
//! Signed `Int` arithmetic that overflows (the bounds' `hi - lo`, the sum of
//! `hi + lo`, the edge's `dy`, `dx`, `point.y - pt1.y` and their product),
//! a `Real`-to-`Int` conversion out of range (the version-1 water area),
//! and a trigger id counter at `Int` maximum (`maxTriggerId++`) are undefined
//! upstream. They are [`PolygonUndefined`] errors here. The parser cannot
//! return one through `ChunkResult`, so it records it in
//! [`TriggerParse::undefined`] and stops.
//!
//! # Not translated
//!
//! - The snapshot methods `crc`, `xfer` and `loadPostProcess` (the save-game
//!   layer; `crc` is empty upstream, `xfer` writes the bounds and radius).
//! - The editor's mutators `setPoint`, `insertPoint`, `deletePoint`,
//!   `removePolygonTrigger`, and the render and selection flags of the
//!   WorldBuilder. (`addPoint` is here: the parse uses it.)
//! - The `ThePolygonTriggerListPtr` memory pool (`newInstance`/`deleteInstance`
//!   are ownership here).
//! - The water handle as a pointer: [`PolygonTrigger::water_handle`] returns
//!   the trigger itself, which is all a `WaterHandle` ever held.
//! - The auto-added water area's name: upstream sets
//!   `"AutoAddedWaterAreaTrigger"` only in `_DEBUG` builds, so it is empty
//!   here, as in the release build.
//! - The ground height of the centre point: `TheTerrainLogic->getGroundHeight`
//!   is a cut point ([`PolygonTrigger::center_point`] returns x and y).
//! - `TheGlobalData`'s water extents: they are passed in ([`TriggerParse::new`]).

use crate::common::base_type::{ICoord2D, ICoord3D, IRegion2D};
use crate::formats::data_chunk::{ChunkResult, DataChunkInfo, DataChunkInput};
use petir::Real;

/// `K_TRIGGERS_VERSION_1` (MapReaderWriterInfo.h): before water areas.
pub const K_TRIGGERS_VERSION_1: u16 = 1;
/// `K_TRIGGERS_VERSION_2`: added `m_isWaterArea`.
pub const K_TRIGGERS_VERSION_2: u16 = 2;
/// `K_TRIGGERS_VERSION_3`: added `m_isRiver` and `m_riverStart`.
pub const K_TRIGGERS_VERSION_3: u16 = 3;
/// `K_TRIGGERS_VERSION_4`: added the layer name. The version the writer uses.
pub const K_TRIGGERS_VERSION_4: u16 = 4;
/// `MAP_XY_FACTOR` (MapObject.h): how wide and tall each height map square is.
pub const MAP_XY_FACTOR: f32 = 10.0;
/// `BIG_INT` of `updateBounds`: the start of the running minimum and maximum.
const BIG_INT: i32 = 0x007f_fff0;

/// Where upstream is undefined (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolygonUndefined {
    /// An `Int` operation overflows (signed overflow, undefined upstream).
    IntOverflow,
    /// A `Real` outside the range of `Int` is converted to `Int`.
    RealToIntOutOfRange,
    /// `maxTriggerId++` (or `maxTriggerId + 1`) past the largest `Int`.
    IdOverflow,
}

impl std::fmt::Display for PolygonUndefined {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolygonUndefined::IntOverflow => f.write_str("Int overflow in a polygon trigger (undefined upstream)"),
            PolygonUndefined::RealToIntOutOfRange => f.write_str("Real out of range of Int (undefined upstream)"),
            PolygonUndefined::IdOverflow => f.write_str("trigger id overflows Int (undefined upstream)"),
        }
    }
}

impl std::error::Error for PolygonUndefined {}

/// `(Int)real` of C: truncation toward zero, undefined when out of range
/// or NaN.
fn real_to_int(v: f32) -> Result<i32, PolygonUndefined> {
    // NaN is in no range, so it is refused with the rest
    if !(-2_147_483_648.0..2_147_483_648.0).contains(&v) {
        return Err(PolygonUndefined::RealToIntOutOfRange);
    }
    Ok(v as i32)
}

/// `Int` addition, undefined when it overflows.
fn int_add(a: i32, b: i32) -> Result<i32, PolygonUndefined> {
    a.checked_add(b).ok_or(PolygonUndefined::IntOverflow)
}

/// `Int` subtraction, undefined when it overflows.
fn int_sub(a: i32, b: i32) -> Result<i32, PolygonUndefined> {
    a.checked_sub(b).ok_or(PolygonUndefined::IntOverflow)
}

/// `class PolygonTrigger` (the data and the queries; see the module docs).
#[derive(Clone, Debug, PartialEq)]
pub struct PolygonTrigger {
    /// `m_triggerName`.
    trigger_name: Vec<u8>,
    /// `m_triggerID`: unique for the map, as far as the file says.
    trigger_id: i32,
    /// `m_points[0 .. m_numPoints]`, closed implicitly (the last to the first).
    points: Vec<ICoord3D>,
    /// `m_riverStart`: the index of the start point of a river.
    river_start: i32,
    /// `m_exportWithScripts`.
    export_with_scripts: bool,
    /// `m_isWaterArea`: the map's water is this polygon.
    is_water_area: bool,
    /// `m_isRiver`.
    is_river: bool,
    /// `m_layerName`: the World Builder layer it was drawn on.
    layer_name: Vec<u8>,
}

impl PolygonTrigger {
    /// `PolygonTrigger(initialAllocation)` with the id given (upstream takes
    /// it from `s_currentID`; see [`PolygonTriggerList::new_trigger`]). The
    /// allocation hint is dropped: a `Vec` grows, and the size is not
    /// observable.
    pub fn new(trigger_id: i32) -> Self {
        PolygonTrigger {
            trigger_name: Vec::new(),
            trigger_id,
            points: Vec::new(),
            river_start: 0,
            export_with_scripts: false,
            is_water_area: false,
            is_river: false,
            layer_name: Vec::new(),
        }
    }

    /// `getTriggerName()`.
    pub fn trigger_name(&self) -> &[u8] {
        &self.trigger_name
    }

    /// `setTriggerName(name)`.
    pub fn set_trigger_name(&mut self, name: &[u8]) {
        self.trigger_name = name.to_vec();
    }

    /// `getLayerName()`.
    pub fn layer_name(&self) -> &[u8] {
        &self.layer_name
    }

    /// `setLayerName(name)`.
    pub fn set_layer_name(&mut self, name: &[u8]) {
        self.layer_name = name.to_vec();
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.trigger_id
    }

    /// `getNumPoints()`.
    pub fn num_points(&self) -> i32 {
        self.points.len() as i32
    }

    /// `getPoint(ndx)` clamps the index to the points. With no points upstream
    /// reads `m_points - 1`, which is undefined, so `None` here.
    pub fn get_point(&self, ndx: i32) -> Option<ICoord3D> {
        let last = self.points.len().checked_sub(1)?;
        let i = (ndx.max(0) as usize).min(last);
        Some(self.points[i])
    }

    /// `addPoint(point)`: at the end of the polygon.
    pub fn add_point(&mut self, point: ICoord3D) {
        self.points.push(point);
    }

    /// `doExportWithScripts()`.
    pub fn export_with_scripts(&self) -> bool {
        self.export_with_scripts
    }

    /// `setDoExportWithScripts(val)`.
    pub fn set_export_with_scripts(&mut self, val: bool) {
        self.export_with_scripts = val;
    }

    /// `isWaterArea()`.
    pub fn is_water_area(&self) -> bool {
        self.is_water_area
    }

    /// `setWaterArea(val)`.
    pub fn set_water_area(&mut self, val: bool) {
        self.is_water_area = val;
    }

    /// `isRiver()`.
    pub fn is_river(&self) -> bool {
        self.is_river
    }

    /// `setRiver(val)`.
    pub fn set_river(&mut self, val: bool) {
        self.is_river = val;
    }

    /// `getRiverStart()`.
    pub fn river_start(&self) -> i32 {
        self.river_start
    }

    /// `setRiverStart(val)`.
    pub fn set_river_start(&mut self, val: i32) {
        self.river_start = val;
    }

    /// `isValid()`: at least one point.
    pub fn is_valid(&self) -> bool {
        !self.points.is_empty()
    }

    /// `getWaterHandle()`: the handle of a water area (the trigger itself,
    /// which is all upstream's `WaterHandle` holds), `None` otherwise.
    pub fn water_handle(&self) -> Option<&PolygonTrigger> {
        if self.is_water_area { Some(self) } else { None }
    }

    /// `updateBounds()` and the `m_bounds`/`m_radius` it leaves: the box of
    /// the points (starting from `BIG_INT`, so an empty polygon gets the
    /// inverted box of upstream's loop, except that the zero box is read
    /// for an empty polygon, see the module docs) and the radius of its
    /// half-extents.
    fn bounds_and_radius(&self) -> Result<(IRegion2D, f32), PolygonUndefined> {
        if self.points.is_empty() {
            // zero-filled memory: the box and the radius were never set
            return Ok((IRegion2D::default(), 0.0));
        }
        let mut lo = ICoord2D { x: BIG_INT, y: BIG_INT };
        let mut hi = ICoord2D { x: -BIG_INT, y: -BIG_INT };
        for p in &self.points {
            if p.x < lo.x {
                lo.x = p.x;
            }
            if p.y < lo.y {
                lo.y = p.y;
            }
            if p.x > hi.x {
                hi.x = p.x;
            }
            if p.y > hi.y {
                hi.y = p.y;
            }
        }
        // Real halfWidth = (hi.x - lo.x) / 2.0f;  Real halfHeight = (hi.y + lo.y) / 2.0f;
        let half_width = (int_sub(hi.x, lo.x)? as f32) / 2.0;
        let half_height = (int_add(hi.y, lo.y)? as f32) / 2.0;
        // m_radius = sqrt(halfHeight*halfHeight + halfWidth*halfWidth): C's double
        // sqrt of the float sum, rounded back once (as in base_type.rs).
        let sum = half_height * half_height + half_width * half_width;
        let radius = Real::sqrt(f64::from(sum)) as f32;
        Ok((IRegion2D { lo, hi }, radius))
    }

    /// `getRadius()`: half the diagonal of the box, as `updateBounds` computes it.
    pub fn radius(&self) -> Result<f32, PolygonUndefined> {
        self.bounds_and_radius().map(|(_, r)| r)
    }

    /// The 2D bounding box (`m_bounds`).
    pub fn bounds(&self) -> Result<IRegion2D, PolygonUndefined> {
        self.bounds_and_radius().map(|(b, _)| b)
    }

    /// `getCenterPoint`'s x and y: the middle of the box. Its z is the ground
    /// height from `TheTerrainLogic` (a cut point), so it is not returned.
    pub fn center_point(&self) -> Result<(f32, f32), PolygonUndefined> {
        let (b, _) = self.bounds_and_radius()?;
        let x = (int_add(b.lo.x, b.hi.x)? as f32) / 2.0;
        let y = (int_add(b.lo.y, b.hi.y)? as f32) / 2.0;
        Ok((x, y))
    }

    /// `pointInTrigger(point)`: even-odd crossing of a ray from the point to
    /// +x, over the edges that span its height, horizontal edges ignored.
    /// Points outside the box are not inside.
    pub fn point_in_trigger(&self, point: ICoord3D) -> Result<bool, PolygonUndefined> {
        let (b, _) = self.bounds_and_radius()?;
        if point.x < b.lo.x || point.y < b.lo.y || point.x > b.hi.x || point.y > b.hi.y {
            return Ok(false);
        }
        let n = self.points.len();
        let mut inside = false;
        for i in 0..n {
            let pt1 = self.points[i];
            let pt2 = if i == n - 1 { self.points[0] } else { self.points[i + 1] };
            if pt1.y == pt2.y {
                continue; // ignore horizontal lines
            }
            if pt1.y < point.y && pt2.y < point.y {
                continue;
            }
            if pt1.y >= point.y && pt2.y >= point.y {
                continue;
            }
            if pt1.x < point.x && pt2.x < point.x {
                continue;
            }
            // Line segment crosses the ray from point x->infinity.
            let dy = int_sub(pt2.y, pt1.y)?;
            let dx = int_sub(pt2.x, pt1.x)?;
            let dyp = int_sub(point.y, pt1.y)?;
            let prod = dx.checked_mul(dyp).ok_or(PolygonUndefined::IntOverflow)?;
            // Real intersectionX = pt1.x + (dx * (point.y-pt1.y)) / ((Real)dy);
            let intersection_x = (pt1.x as f32) + (prod as f32) / (dy as f32);
            if intersection_x >= point.x as f32 {
                inside = !inside;
            }
        }
        Ok(inside)
    }
}

/// The statics of `PolygonTrigger`: the chain of triggers and the next id.
#[derive(Clone, Debug, PartialEq)]
pub struct PolygonTriggerList {
    /// The chain, head first (`ThePolygonTriggerListPtr` and `m_nextPolygonTrigger`).
    triggers: Vec<PolygonTrigger>,
    /// `s_currentID`: the id the next `PolygonTrigger` gets.
    current_id: i32,
}

impl Default for PolygonTriggerList {
    fn default() -> Self {
        Self::new()
    }
}

impl PolygonTriggerList {
    /// An empty chain, `s_currentID = 1`.
    pub fn new() -> Self {
        PolygonTriggerList { triggers: Vec::new(), current_id: 1 }
    }

    /// `getFirstPolygonTrigger()`.
    pub fn first(&self) -> Option<&PolygonTrigger> {
        self.triggers.first()
    }

    /// The chain, head first: `getFirstPolygonTrigger()` then `getNext()`.
    pub fn iter(&self) -> impl Iterator<Item = &PolygonTrigger> {
        self.triggers.iter()
    }

    /// The number of triggers in the chain.
    pub fn len(&self) -> usize {
        self.triggers.len()
    }

    /// Whether the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.triggers.is_empty()
    }

    /// `s_currentID`: the id the next trigger gets.
    pub fn next_id(&self) -> i32 {
        self.current_id
    }

    /// `getPolygonTriggerByID(triggerID)`: the first trigger with that id.
    pub fn by_id(&self, trigger_id: i32) -> Option<&PolygonTrigger> {
        self.triggers.iter().find(|t| t.trigger_id == trigger_id)
    }

    /// `newInstance(PolygonTrigger)(n)`: a trigger with the next id. Upstream
    /// adds one to `s_currentID` in the constructor, so a trigger that is
    /// dropped still uses an id, as there.
    pub fn new_trigger(&mut self) -> PolygonTrigger {
        let id = self.current_id;
        // s_currentID++: the counter reaches Int maximum only after 2^31
        // triggers, which a chunk file cannot hold (each needs bytes).
        self.current_id = self.current_id.checked_add(1).expect("s_currentID overflows Int (undefined upstream)");
        PolygonTrigger::new(id)
    }

    /// `addPolygonTrigger(t)`: at the head of the chain.
    pub fn add_polygon_trigger(&mut self, t: PolygonTrigger) {
        self.triggers.insert(0, t);
    }

    /// `setNextPoly(t)` on the last trigger: at the end of the chain.
    pub fn append(&mut self, t: PolygonTrigger) {
        self.triggers.push(t);
    }

    /// `deleteTriggers()`: the chain is emptied and `s_currentID` is 1 again.
    pub fn delete_triggers(&mut self) {
        self.triggers.clear();
        self.current_id = 1;
    }
}

/// The context of [`parse_polygon_triggers_data_chunk`]: the list it fills,
/// the water extents of `TheGlobalData` (which the version-1 path reads), and
/// the undefined input it stopped on, if any.
#[derive(Clone, Debug, PartialEq)]
pub struct TriggerParse {
    /// The list the parse fills.
    pub list: PolygonTriggerList,
    /// `TheGlobalData->m_waterExtentX`.
    water_extent_x: f32,
    /// `TheGlobalData->m_waterExtentY`.
    water_extent_y: f32,
    /// Set when the parse stopped on an undefined input (see the module docs).
    pub undefined: Option<PolygonUndefined>,
}

impl TriggerParse {
    /// A parse with the given water extents. The version-1 path converts
    /// `30 * MAP_XY_FACTOR + extent` to `Int`, so an extent that cannot be
    /// converted is refused here, where the error can be returned, instead of
    /// in the middle of the chunk.
    pub fn new(water_extent_x: f32, water_extent_y: f32) -> Result<Self, PolygonUndefined> {
        real_to_int(30.0 * MAP_XY_FACTOR + water_extent_x)?;
        real_to_int(30.0 * MAP_XY_FACTOR + water_extent_y)?;
        Ok(TriggerParse { list: PolygonTriggerList::new(), water_extent_x, water_extent_y, undefined: None })
    }
}

/// `PolygonTrigger::ParsePolygonTriggersDataChunk`: reads the `PolygonTriggers`
/// chunk into `ctx.list`. Versions 1 to 4, the newer format for 4. A version-1
/// file gets a water area around the map, with the points upstream gives it.
/// Returns `Ok(false)` and sets `ctx.undefined` on an undefined input (the
/// chunk reader reports the read errors as `Err`, as always).
pub fn parse_polygon_triggers_data_chunk(
    file: &mut DataChunkInput<TriggerParse>,
    info: &DataChunkInfo,
    ctx: &mut TriggerParse,
) -> ChunkResult<bool> {
    let version = info.version;
    // Remove any existing polygon triggers, if any.
    ctx.list.delete_triggers();
    let mut max_trigger_id: i32 = 0;
    // pPrevTrig: the last trigger added to the chain, if any.
    let mut have_prev = false;

    let mut count = file.read_int()?;
    while count > 0 {
        count -= 1;
        let trigger_name = file.read_ascii_string()?;
        let layer_name = if version >= K_TRIGGERS_VERSION_4 { file.read_ascii_string()? } else { Vec::new() };
        let trigger_id = file.read_int()?;
        let is_water = if version >= K_TRIGGERS_VERSION_2 { file.read_byte()? != 0 } else { false };
        let (is_river, river_start) = if version >= K_TRIGGERS_VERSION_3 {
            let r = file.read_byte()? != 0;
            (r, file.read_int()?)
        } else {
            (false, 0)
        };
        let num_points = file.read_int()?;

        let mut trig = ctx.list.new_trigger();
        trig.set_trigger_name(&trigger_name);
        if version >= K_TRIGGERS_VERSION_4 {
            trig.set_layer_name(&layer_name);
        }
        trig.set_water_area(is_water);
        trig.set_river(is_river);
        trig.set_river_start(river_start);
        trig.trigger_id = trigger_id;
        max_trigger_id = max_trigger_id.max(trigger_id);
        for _ in 0..num_points.max(0) {
            let x = file.read_int()?;
            let y = file.read_int()?;
            let z = file.read_int()?;
            trig.add_point(ICoord3D { x, y, z });
        }
        if num_points < 2 {
            // "Deleting polygon trigger with %d points": dropped, and it does
            // not become the previous trigger.
            continue;
        }
        if have_prev {
            ctx.list.append(trig);
        } else {
            ctx.list.add_polygon_trigger(trig);
        }
        have_prev = true;
    }

    if version == K_TRIGGERS_VERSION_1 {
        // before water areas existed, so create a default one.
        let mut trig = ctx.list.new_trigger();
        trig.set_water_area(true);
        // m_triggerID = maxTriggerId++;
        let Some(next_max) = max_trigger_id.checked_add(1) else {
            ctx.undefined = Some(PolygonUndefined::IdOverflow);
            return Ok(false);
        };
        trig.trigger_id = max_trigger_id;
        max_trigger_id = next_max;
        // The four corners of the map, at the old water height (7); the
        // extents are the ones validated in TriggerParse::new.
        let x0 = (-30.0 * MAP_XY_FACTOR) as i32;
        let y0 = (-30.0 * MAP_XY_FACTOR) as i32;
        let x1 = real_to_int(30.0 * MAP_XY_FACTOR + ctx.water_extent_x).expect("checked in TriggerParse::new");
        let y1 = real_to_int(30.0 * MAP_XY_FACTOR + ctx.water_extent_y).expect("checked in TriggerParse::new");
        let z = 7;
        trig.add_point(ICoord3D { x: x0, y: y0, z });
        trig.add_point(ICoord3D { x: x1, y: y0, z });
        trig.add_point(ICoord3D { x: x1, y: y1, z });
        trig.add_point(ICoord3D { x: x0, y: y1, z });
        if have_prev {
            ctx.list.append(trig);
        } else {
            ctx.list.add_polygon_trigger(trig);
        }
    }

    // s_currentID = maxTriggerId+1;
    let Some(next_id) = max_trigger_id.checked_add(1) else {
        ctx.undefined = Some(PolygonUndefined::IdOverflow);
        return Ok(false);
    };
    ctx.list.current_id = next_id;
    Ok(true)
}

/// `PolygonTrigger::WritePolygonTriggersDataChunk`: the `PolygonTriggers`
/// chunk at version 4, appended to `out`. `id` is the chunk's id, which
/// upstream's `DataChunkOutput` takes from its table of contents (`allocateID`,
/// not translated; the caller supplies it). Layout: `id` (u32), version (u16),
/// data size (i32), then the count (i32) and, per trigger, the name and the
/// layer name (u16 length and bytes), the id (i32), water and river (one byte
/// each), the river start (i32), the point count (i32) and each point's x, y,
/// z (i32). Strings longer than 65535 bytes are cut to the length the u16
/// holds, as upstream's `writeAsciiString` does.
pub fn write_polygon_triggers_chunk(list: &PolygonTriggerList, id: u32, out: &mut Vec<u8>) {
    let header = out.len();
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(&K_TRIGGERS_VERSION_4.to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes()); // the size, written below
    let body = out.len();

    write_int(out, list.len() as i32);
    for t in list.iter() {
        write_ascii_string(out, &t.trigger_name);
        write_ascii_string(out, &t.layer_name);
        write_int(out, t.trigger_id);
        out.push(u8::from(t.is_water_area));
        out.push(u8::from(t.is_river));
        write_int(out, t.river_start);
        write_int(out, t.points.len() as i32);
        for p in &t.points {
            write_int(out, p.x);
            write_int(out, p.y);
            write_int(out, p.z);
        }
    }
    let size = (out.len() - body) as i32;
    out[header + 6..header + 10].copy_from_slice(&size.to_le_bytes());
}

fn write_int(out: &mut Vec<u8>, v: i32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_ascii_string(out: &mut Vec<u8>, s: &[u8]) {
    let len = s.len() as u16;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&s[..usize::from(len)]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::data_chunk::{CachedFileInputStream, ChunkError, ChunkUndefined};

    fn pt(x: i32, y: i32) -> ICoord3D {
        ICoord3D { x, y, z: 0 }
    }

    /// A square, counter-clockwise, from (0,0) to (10,10).
    fn square() -> PolygonTrigger {
        let mut t = PolygonTrigger::new(1);
        for p in [pt(0, 0), pt(10, 0), pt(10, 10), pt(0, 10)] {
            t.add_point(p);
        }
        t
    }

    #[test]
    fn inside_outside_and_edges() {
        let t = square();
        assert_eq!(t.point_in_trigger(pt(5, 5)), Ok(true));
        assert_eq!(t.point_in_trigger(pt(-1, 5)), Ok(false));
        assert_eq!(t.point_in_trigger(pt(11, 5)), Ok(false));
        assert_eq!(t.point_in_trigger(pt(5, 11)), Ok(false));
        assert_eq!(t.point_in_trigger(pt(5, -1)), Ok(false));
    }

    #[test]
    fn bounds_radius_and_centre() {
        let t = square();
        assert_eq!(t.bounds(), Ok(IRegion2D { lo: ICoord2D { x: 0, y: 0 }, hi: ICoord2D { x: 10, y: 10 } }));
        // halfWidth = halfHeight = 5, radius = sqrt(50)
        assert_eq!(t.radius().unwrap().to_bits(), (50.0f32.sqrt()).to_bits());
        assert_eq!(t.center_point(), Ok((5.0, 5.0)));
    }

    #[test]
    fn empty_trigger_reads_the_zero_box() {
        let t = PolygonTrigger::new(7);
        assert_eq!(t.bounds(), Ok(IRegion2D::default()));
        assert_eq!(t.radius(), Ok(0.0));
        assert_eq!(t.point_in_trigger(pt(0, 0)), Ok(false));
        assert!(!t.is_valid());
        assert_eq!(t.get_point(0), None);
    }

    #[test]
    fn get_point_clamps_like_upstream() {
        let t = square();
        assert_eq!(t.get_point(-3), Some(pt(0, 0)));
        assert_eq!(t.get_point(2), Some(pt(10, 10)));
        assert_eq!(t.get_point(99), Some(pt(0, 10)));
    }

    #[test]
    fn overflow_is_an_error_not_a_value() {
        let mut t = PolygonTrigger::new(1);
        t.add_point(pt(i32::MIN, 0));
        t.add_point(pt(i32::MAX, 0));
        t.add_point(pt(0, 10));
        // hi.x - lo.x overflows
        assert_eq!(t.bounds(), Err(PolygonUndefined::IntOverflow));
        assert_eq!(t.point_in_trigger(pt(0, 5)), Err(PolygonUndefined::IntOverflow));
    }

    #[test]
    fn real_to_int_range() {
        assert_eq!(real_to_int(-2_147_483_648.0), Ok(i32::MIN));
        assert_eq!(real_to_int(2_147_483_520.0), Ok(2_147_483_520));
        assert_eq!(real_to_int(2_147_483_648.0), Err(PolygonUndefined::RealToIntOutOfRange));
        assert_eq!(real_to_int(f32::NAN), Err(PolygonUndefined::RealToIntOutOfRange));
        assert_eq!(real_to_int(-7.9), Ok(-7));
    }

    /// `PolygonTriggers` chunk bytes: a table of contents naming the chunk
    /// (id 1), then one chunk of the given version and body.
    fn file(version: u16, body: &[u8]) -> Vec<u8> {
        let mut v = b"CkMp".to_vec();
        v.extend_from_slice(&1i32.to_le_bytes());
        v.push(b"PolygonTriggers".len() as u8);
        v.extend_from_slice(b"PolygonTriggers");
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&version.to_le_bytes());
        v.extend_from_slice(&(body.len() as i32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    fn put_str(v: &mut Vec<u8>, s: &[u8]) {
        v.extend_from_slice(&(s.len() as u16).to_le_bytes());
        v.extend_from_slice(s);
    }

    fn parse(bytes: Vec<u8>, wx: f32, wy: f32) -> (ChunkResult<bool>, TriggerParse) {
        let mut input = DataChunkInput::new(CachedFileInputStream::new(bytes).unwrap()).unwrap();
        input.register_parser(b"PolygonTriggers", b"", parse_polygon_triggers_data_chunk);
        let mut ctx = TriggerParse::new(wx, wy).unwrap();
        let r = input.parse(&mut ctx);
        (r, ctx)
    }

    #[test]
    fn version_4_chunk_parses_in_order_and_writes_back() {
        let mut body = Vec::new();
        body.extend_from_slice(&2i32.to_le_bytes());
        for (name, layer, id, water, river, start, pts) in [
            (&b"Lake"[..], &b"Water"[..], 5i32, 1u8, 0u8, 0i32, vec![(0i32, 0i32), (4, 0), (4, 4)]),
            (b"Road", b"", 9, 0, 1, 2, vec![(-1i32, -1i32), (3, 3)]),
        ] {
            put_str(&mut body, name);
            put_str(&mut body, layer);
            body.extend_from_slice(&id.to_le_bytes());
            body.push(water);
            body.push(river);
            body.extend_from_slice(&start.to_le_bytes());
            body.extend_from_slice(&(pts.len() as i32).to_le_bytes());
            for (x, y) in pts {
                body.extend_from_slice(&x.to_le_bytes());
                body.extend_from_slice(&y.to_le_bytes());
                body.extend_from_slice(&0i32.to_le_bytes());
            }
        }
        let (r, ctx) = parse(file(4, &body), 0.0, 0.0);
        assert_eq!(r, Ok(true));
        assert_eq!(ctx.undefined, None);
        let names: Vec<(&[u8], i32)> = ctx.list.iter().map(|t| (t.trigger_name(), t.id())).collect();
        assert_eq!(names, vec![(&b"Lake"[..], 5), (&b"Road"[..], 9)]);
        assert_eq!(ctx.list.next_id(), 10);
        let lake = ctx.list.first().unwrap();
        assert_eq!(lake.layer_name(), b"Water");
        assert!(lake.is_water_area());
        assert_eq!(lake.water_handle().map(|t| t.id()), Some(5));
        assert_eq!(ctx.list.by_id(9).unwrap().river_start(), 2);
        assert!(ctx.list.by_id(9).unwrap().is_river());
        // write and parse again: the same triggers
        let mut out = Vec::new();
        write_polygon_triggers_chunk(&ctx.list, 1, &mut out);
        assert_eq!(&out[..6], &[1, 0, 0, 0, 4, 0]);
        let body_back = &out[10..];
        assert_eq!(i32::from_le_bytes(out[6..10].try_into().unwrap()) as usize, body_back.len());
        assert_eq!(body_back, &body[..], "the writer produces the reader's bytes");
    }

    #[test]
    fn version_1_adds_the_water_area_at_the_map_corners() {
        let mut body = Vec::new();
        body.extend_from_slice(&1i32.to_le_bytes());
        put_str(&mut body, b"T");
        body.extend_from_slice(&3i32.to_le_bytes()); // id
        body.extend_from_slice(&3i32.to_le_bytes()); // points
        for v in [0i32, 0, 0, 1, 0, 0, 1, 1, 0] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        let (r, ctx) = parse(file(1, &body), 2.5, -3.5);
        assert_eq!(r, Ok(true));
        // upstream gives the auto area maxTriggerId++ (3, then 4): the id is
        // repeated, and by_id finds the first, the file's trigger
        let ids: Vec<i32> = ctx.list.iter().map(|t| t.id()).collect();
        assert_eq!(ids, vec![3, 3]);
        assert!(!ctx.list.by_id(3).unwrap().is_water_area());
        let auto = ctx.list.iter().last().unwrap();
        assert!(auto.is_water_area());
        let pts: Vec<(i32, i32, i32)> = auto.points.iter().map(|p| (p.x, p.y, p.z)).collect();
        assert_eq!(pts, vec![(-300, -300, 7), (302, -300, 7), (302, 296, 7), (-300, 296, 7)]);
        assert_eq!(ctx.list.next_id(), 5);
    }

    #[test]
    fn short_triggers_are_dropped_and_the_read_is_still_done() {
        let mut body = Vec::new();
        body.extend_from_slice(&2i32.to_le_bytes());
        for (id, n) in [(1i32, 1i32), (2, -4)] {
            put_str(&mut body, b"x");
            put_str(&mut body, b"");
            body.extend_from_slice(&id.to_le_bytes());
            body.push(0);
            body.push(0);
            body.extend_from_slice(&0i32.to_le_bytes());
            body.extend_from_slice(&n.to_le_bytes());
            if n > 0 {
                body.extend_from_slice(&[0u8; 12]);
            }
        }
        let (r, ctx) = parse(file(4, &body), 0.0, 0.0);
        assert_eq!(r, Ok(true));
        assert!(ctx.list.is_empty());
        // both ids were taken from the counter, as upstream's constructor does
        assert_eq!(ctx.list.next_id(), 3);
    }

    #[test]
    fn a_truncated_chunk_is_a_read_error() {
        let mut body = Vec::new();
        body.extend_from_slice(&1i32.to_le_bytes());
        put_str(&mut body, b"T");
        let (r, _) = parse(file(4, &body), 0.0, 0.0);
        assert_eq!(r, Err(ChunkError::Undefined(ChunkUndefined::UninitialisedRead)));
    }

    #[test]
    fn the_version_one_id_overflow_stops_the_parse() {
        // one trigger with id Int maximum and no points (dropped): the auto
        // water area needs maxTriggerId + 1, which overflows
        let mut body = Vec::new();
        body.extend_from_slice(&1i32.to_le_bytes());
        put_str(&mut body, b"T");
        body.extend_from_slice(&i32::MAX.to_le_bytes());
        body.extend_from_slice(&0i32.to_le_bytes());
        let (r, ctx) = parse(file(1, &body), 0.0, 0.0);
        assert_eq!(r, Ok(false));
        assert_eq!(ctx.undefined, Some(PolygonUndefined::IdOverflow));
    }

    #[test]
    fn an_extent_that_cannot_convert_is_refused_up_front() {
        assert!(TriggerParse::new(f32::NAN, 0.0).is_err());
        assert!(TriggerParse::new(0.0, 3.0e9).is_err());
        assert!(TriggerParse::new(1.0e9, 0.0).is_ok());
    }
}
