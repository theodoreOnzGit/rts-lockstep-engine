//! PARTLY DIFFERENTIAL-TESTED (#215): matches the upstream C++ (tests/differential_draft_cull.rs, 24,000 random programs, with the structure dumped after every operation).
//! NOT covered: `save`, `load`, `load_object_linkage`, `save_object_linkage` (chunk I/O) and the typed `get_*_collected_object` wrappers (the same as `peek_*`). Not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/gridcull.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/gridcull.cpp
//!
//! `GridCullSystemClass`, `GridLinkClass`, `GridListIterator` (a plain walk of `Next` here). See `cullsys.rs` for how
//! the pointers became handles. `Cells` is a `Vec<Option<CullableHandle>>` of list heads; `NoGridList` likewise.
//! Upstream notes kept: `compute_box` builds the box with `Init((min+max)*0.5, (min-max)*0.5)`, a NEGATIVE extent;
//! `UNGRIDDED_ADDRESS = 0xFFFFFFFF` is compared with the `int` `GridAddress` (so it is `-1`); an object larger than
//! `MaxObjExtent` has `GridAddress == UNGRIDDED_ADDRESS`, which differs from the cell address `Update_Culling` computes,
//! so such an object is unlinked and relinked every update; the destructor `delete`s `Cells` without unlinking the
//! objects; `map_point_to_cell` converts `floor(...)` straight to `int` (Rust's `as i32` saturates where C is undefined).

use crate::logic::modules::plant;
use super::aabox::AABoxClass;
use super::chunk_save::ChunkSaveClass;
use super::colmath::OverlapType;
use super::colmathaabox::overlap_test_aabox_aabox;
use super::colmathfrustum::overlap_test_frustum_aabox;
use super::colmathobbox::overlap_test_obbox_aabox;
use super::cullsys::{CullLinkClass, CullSystemClass, CullSystemId, CullableHandle};
use super::frustum::FrustumClass;
use super::lineseg::LineSegClass;
use super::obbox::OBBoxClass;
use crate::ww::chunkio::{ChunkIoResult, ChunkLoad};
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `const uint32 GRID_CURRENT_VERSION = 0x00010000;`
pub const GRID_CURRENT_VERSION: u32 = 0x00010000;
/// `GRID_CHUNK_VERSION = 1`, `GRID_CHUNK_PARAMETERS = 0x100`.
pub const GRID_CHUNK_VERSION: u32 = 0x00000001;
pub const GRID_CHUNK_PARAMETERS: u32 = 0x00000100;

/// `TERMINATION_CELL_COUNT = 16384`: "algorithm terminates if we ever have more than this many cells".
pub const TERMINATION_CELL_COUNT: i32 = 16384;
/// `UNGRIDDED_ADDRESS = 0xFFFFFFFF`: "address given to objs that didn't fit in grid" (as the `int` it is compared with).
pub const UNGRIDDED_ADDRESS: i32 = -1;

/// `struct IOGridParametersStruct` (gridcull.cpp:53): 3 + 3 + 3 floats, 3 `uint32`, 1 float: 52 bytes.
#[derive(Clone, Copy, Debug, Default)]
pub struct IOGridParametersStruct {
    pub min_cell_size: [f32; 3],
    pub origin: [f32; 3],
    pub cell_dim: [f32; 3],
    pub cell_count: [u32; 3],
    pub max_obj_extent: f32,
}

impl IOGridParametersStruct {
    /// Little-endian image of the struct, as `csave.Write(&params, sizeof(params))` writes it.
    pub fn to_bytes(&self) -> [u8; 52] {
        let mut b = [0u8; 52];
        for i in 0..3 {
            b[i * 4..i * 4 + 4].copy_from_slice(&self.min_cell_size[i].to_le_bytes());
            b[12 + i * 4..12 + i * 4 + 4].copy_from_slice(&self.origin[i].to_le_bytes());
            b[24 + i * 4..24 + i * 4 + 4].copy_from_slice(&self.cell_dim[i].to_le_bytes());
            b[36 + i * 4..36 + i * 4 + 4].copy_from_slice(&self.cell_count[i].to_le_bytes());
        }
        b[48..52].copy_from_slice(&self.max_obj_extent.to_le_bytes());
        b
    }

    /// Inverse of [`IOGridParametersStruct::to_bytes`].
    pub fn from_bytes(b: &[u8; 52]) -> Self {
        let f = |i: usize| f32::from_le_bytes([b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]]);
        let u = |i: usize| u32::from_le_bytes([b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]]);
        IOGridParametersStruct { min_cell_size: [f(0), f(1), f(2)], origin: [f(3), f(4), f(5)], cell_dim: [f(6), f(7), f(8)], cell_count: [u(9), u(10), u(11)], max_obj_extent: f(12) }
    }
}

/// `class GridLinkClass : public CullLinkClass`.
#[derive(Clone, Debug)]
pub struct GridLinkClass {
    /// `CullLinkClass::System`.
    pub system: Option<CullSystemId>,
    /// `GridAddress`: address in the grid (initially -1).
    pub grid_address: i32,
    /// `Prev`: prev object in this cell.
    pub prev: Option<CullableHandle>,
    /// `Next`: next object in this cell.
    pub next: Option<CullableHandle>,
}

impl GridLinkClass {
    /// `GridLinkClass(system)`.
    pub fn new(system: CullSystemId) -> Self {
        GridLinkClass { system: Some(system), grid_address: -1, prev: None, next: None }
    }
}

fn link_of<R>(obj: &CullableHandle, f: impl FnOnce(&GridLinkClass) -> R) -> R {
    match obj.read().unwrap().get_cull_link() {
        Some(CullLinkClass::Grid(l)) => f(l),
        _ => panic!("object has no grid link"),
    }
}

fn link_of_mut<R>(obj: &CullableHandle, f: impl FnOnce(&mut GridLinkClass) -> R) -> R {
    match obj.write().unwrap().get_cull_link_mut() {
        Some(CullLinkClass::Grid(l)) => f(l),
        _ => panic!("object has no grid link"),
    }
}

/// `get_next_object(obj)`: `((GridLinkClass *)obj->Get_Cull_Link())->Next`.
fn get_next_object(obj: &CullableHandle) -> Option<CullableHandle> {
    link_of(obj, |l| l.next.clone())
}

fn cull_box_of(obj: &CullableHandle) -> AABoxClass {
    *obj.read().unwrap().get_cull_box()
}

/// `GridCullSystemClass::StatsStruct`.
#[derive(Clone, Copy, Debug, Default)]
pub struct StatsStruct {
    pub node_count: i32,
    pub nodes_accepted: i32,
    pub nodes_trivially_accepted: i32,
    pub nodes_rejected: i32,
}

/// `GridCullSystemClass::VolumeStruct`: cells `Min[0..3]` to `Max[0..3] - 1`.
#[derive(Clone, Copy, Debug, Default)]
pub struct VolumeStruct {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

impl VolumeStruct {
    /// `VolumeStruct(i0, j0, k0, i1, j1, k1)` (asserts `Max > Min` per axis).
    pub fn new(i0: i32, j0: i32, k0: i32, i1: i32, j1: i32, k1: i32) -> Self {
        assert!(i1 > i0);
        assert!(j1 > j0);
        assert!(k1 > k0);
        VolumeStruct { min: [i0, j0, k0], max: [i1, j1, k1] }
    }

    /// `Is_Leaf()`.
    pub fn is_leaf(&self) -> bool {
        self.max[0] - self.min[0] == 1 && self.max[1] - self.min[1] == 1 && self.max[2] - self.min[2] == 1
    }

    /// `Is_Empty()`.
    pub fn is_empty(&self) -> bool {
        self.max[0] - self.min[0] <= 0 || self.max[1] - self.min[1] <= 0 || self.max[2] - self.min[2] <= 0
    }

    /// `Split(v0, v1)`: perpendicular to the longest dimension, halving it (`>> 1`).
    pub fn split(&self) -> (VolumeStruct, VolumeStruct) {
        let mut split_axis = 0usize;
        let delta = [self.max[0] - self.min[0], self.max[1] - self.min[1], self.max[2] - self.min[2]];
        if delta[1] > delta[split_axis] {
            split_axis = 1;
        }
        if delta[2] > delta[split_axis] {
            split_axis = 2;
        }
        assert!(delta[split_axis] > 0);

        let mut v0 = *self;
        let mut v1 = *self;
        let mid = self.min[split_axis] + (delta[split_axis] >> 1);
        v0.max[split_axis] = mid;
        v1.min[split_axis] = mid;
        (v0, v1)
    }
}

/// (test support, #215; not upstream) A view of [`GridCullSystemClass`]'s private state.
#[doc(hidden)]
pub struct GridDebug {
    pub max_obj_extent: f32,
    pub origin: Vector3,
    pub cell_dim: Vector3,
    pub oo_cell_dim: Vector3,
    pub cell_count: [i32; 3],
    pub cells: Vec<Option<CullableHandle>>,
    pub no_grid_list: Option<CullableHandle>,
}

/// `class GridCullSystemClass : public CullSystemClass`.
#[derive(Debug)]
pub struct GridCullSystemClass {
    pub base: CullSystemClass,
    /// `MinCellSize`: min dimensions for a cell (don't go below this...).
    min_cell_size: Vector3,
    /// `MaxObjExtent`: max extent/radius (objects bigger than this are just put in a list).
    max_obj_extent: f32,
    termination_cell_count: i32,
    origin: Vector3,
    cell_dim: Vector3,
    oo_cell_dim: Vector3,
    cell_count: [i32; 3],
    /// `Cells`: heads of the per-cell object lists.
    cells: Vec<Option<CullableHandle>>,
    /// `NoGridList`: objs that were outside or too big for the grid.
    no_grid_list: Option<CullableHandle>,
    /// `ObjCount`: number of objects in the system.
    obj_count: i32,
    stats: StatsStruct,
}

impl GridCullSystemClass {
    /// `GridCullSystemClass()`: `MinCellSize(10,10,10)`, `MaxObjExtent(15)`, `Origin(-100,-100,-100)`,
    /// `CellDim(10,10,10)`, then `Re_Partition((-100..100), 15)` and `Reset_Statistics()`.
    pub fn new() -> Self {
        let mut s = GridCullSystemClass {
            base: CullSystemClass::new(),
            min_cell_size: Vector3::new(10.0, 10.0, 10.0),
            max_obj_extent: 15.0,
            termination_cell_count: TERMINATION_CELL_COUNT,
            origin: Vector3::new(-100.0, -100.0, -100.0),
            cell_dim: Vector3::new(10.0, 10.0, 10.0),
            oo_cell_dim: Vector3::new(0.0, 0.0, 0.0),
            cell_count: [0, 0, 0],
            cells: Vec::new(),
            no_grid_list: None,
            obj_count: 0,
            stats: StatsStruct::default(),
        };
        s.re_partition(&Vector3::new(-100.0, -100.0, -100.0), &Vector3::new(100.0, 100.0, 100.0), 15.0);
        s.reset_statistics();
        s
    }

    /// (test support, #215; not upstream) The grid's private state, for the differential test's structure dump.
    #[doc(hidden)]
    pub fn debug_state(&self) -> GridDebug {
        GridDebug {
            max_obj_extent: self.max_obj_extent,
            origin: self.origin,
            cell_dim: self.cell_dim,
            oo_cell_dim: self.oo_cell_dim,
            cell_count: self.cell_count,
            cells: self.cells.clone(),
            no_grid_list: self.no_grid_list.clone(),
        }
    }

    /// (test support, #215; not upstream) `map_point_to_address(pt, address)` (private upstream, protected here).
    #[doc(hidden)]
    pub fn debug_map_point_to_address(&self, pt: &Vector3) -> (i32, bool) {
        self.map_point_to_address(pt)
    }

    /// `Get_Object_Count()`.
    pub fn get_object_count(&self) -> i32 {
        self.obj_count
    }

    /// `Get_Min_Cell_Size`.
    pub fn get_min_cell_size(&self) -> Vector3 {
        self.min_cell_size
    }

    /// `Set_Min_Cell_Size`.
    pub fn set_min_cell_size(&mut self, size: &Vector3) {
        self.min_cell_size = *size;
        if plant(11343) {
            self.min_cell_size.z = 10.0;
        }
    }

    /// `Get_Termination_Count`.
    pub fn get_termination_count(&self) -> i32 {
        self.termination_cell_count
    }

    /// `Set_Termination_Count`.
    pub fn set_termination_count(&mut self, count: i32) {
        self.termination_cell_count = count;
    }

    /// `Reset_Statistics()`: "number of (virtual) nodes = 2n-1".
    pub fn reset_statistics(&mut self) {
        self.stats.node_count = 2 * (self.cell_count[0] * self.cell_count[1] * self.cell_count[2]) - 1;
        self.stats.nodes_accepted = 0;
        self.stats.nodes_trivially_accepted = 0;
        self.stats.nodes_rejected = 0;
    }

    /// `Get_Statistics()`.
    pub fn get_statistics(&self) -> &StatsStruct {
        &self.stats
    }

    /// `clamp_indices_to_grid(&i, &j, &k)`.
    fn clamp_indices_to_grid(&self, idx: &mut [i32; 3]) {
        for a in 0..3 {
            if idx[a] < 0 {
                idx[a] = 0;
            }
            if idx[a] >= self.cell_count[a] {
                idx[a] = self.cell_count[a] - 1;
            }
        }
    }

    /// `map_point_to_cell(pt, i, j, k)`: the cell indices (even when outside) and whether the point is inside the grid.
    fn map_point_to_cell(&self, pt: &Vector3) -> ([i32; 3], bool) {
        let dp = *pt - self.origin;
        let i = if plant(11331) { (dp.x * self.oo_cell_dim.x) as i32 } else { wwmath::floor(dp.x * self.oo_cell_dim.x) as i32 };
        let j = wwmath::floor(dp.y * self.oo_cell_dim.y) as i32;
        let k = wwmath::floor(dp.z * self.oo_cell_dim.z) as i32;

        let inside = i >= 0 && j >= 0 && k >= 0 && i < self.cell_count[0] && j < self.cell_count[1] && k < self.cell_count[2];
        ([i, j, k], inside)
    }

    /// `map_point_to_address(pt, address)`: `UNGRIDDED_ADDRESS` outside the grid.
    fn map_point_to_address(&self, pt: &Vector3) -> (i32, bool) {
        let (idx, res) = self.map_point_to_cell(pt);
        if res {
            (self.map_indices_to_address(idx[0], idx[1], idx[2]), true)
        } else {
            (UNGRIDDED_ADDRESS, false)
        }
    }

    /// `map_indices_to_address(i, j, k)`.
    fn map_indices_to_address(&self, i: i32, j: i32, k: i32) -> i32 {
        if plant(11332) {
            return i + j * self.cell_count[1] + k * self.cell_count[0] * self.cell_count[1];
        }
        i + j * self.cell_count[0] + k * self.cell_count[0] * self.cell_count[1]
    }

    /// `total_cell_count()`.
    fn total_cell_count(&self) -> i32 {
        self.cell_count[0] * self.cell_count[1] * self.cell_count[2]
    }

    /// `compute_box(i, j, k, &box)`: UPSTREAM DEFECT KEPT, `Init((min+max)*0.5, (min-max)*0.5)` gives a negative extent.
    pub fn compute_box(&self, i: i32, j: i32, k: i32) -> AABoxClass {
        assert!(i >= 0 && j >= 0 && k >= 0);
        assert!(i < self.cell_count[0] && j < self.cell_count[1] && k < self.cell_count[2]);
        let mut min = Vector3::new(0.0, 0.0, 0.0);
        let mut max = Vector3::new(0.0, 0.0, 0.0);
        min.x = self.origin.x + i as f32 * self.cell_dim.x - self.max_obj_extent;
        min.y = self.origin.y + j as f32 * self.cell_dim.y - self.max_obj_extent;
        min.z = self.origin.z + k as f32 * self.cell_dim.z - self.max_obj_extent;
        max.x = min.x + self.cell_dim.x + 2.0f32 * self.max_obj_extent;
        max.y = min.y + self.cell_dim.y + 2.0f32 * self.max_obj_extent;
        max.z = min.z + self.cell_dim.z + 2.0f32 * self.max_obj_extent;

        let mut b = AABoxClass::default();
        b.init(&((min + max) * 0.5f32), &((min - max) * 0.5f32));
        b
    }

    /// `compute_box(vol, &box)`.
    pub fn compute_box_volume(&self, vol: &VolumeStruct) -> AABoxClass {
        assert!(vol.min[0] >= 0 && vol.min[1] >= 0 && vol.min[2] >= 0);
        assert!(vol.max[0] <= self.cell_count[0] && vol.max[1] <= self.cell_count[1] && vol.max[2] <= self.cell_count[2]);
        let min = Vector3::new(
            self.origin.x + vol.min[0] as f32 * self.cell_dim.x - self.max_obj_extent,
            self.origin.y + vol.min[1] as f32 * self.cell_dim.y - self.max_obj_extent,
            self.origin.z + vol.min[2] as f32 * self.cell_dim.z - self.max_obj_extent,
        );
        let max = Vector3::new(
            self.origin.x + vol.max[0] as f32 * self.cell_dim.x + self.max_obj_extent,
            self.origin.y + vol.max[1] as f32 * self.cell_dim.y + self.max_obj_extent,
            self.origin.z + vol.max[2] as f32 * self.cell_dim.z + self.max_obj_extent,
        );
        let center = Vector3::new((max.x + min.x) * 0.5f32, (max.y + min.y) * 0.5f32, (max.z + min.z) * 0.5f32);
        let extent = Vector3::new((max.x - min.x) * 0.5f32, (max.y - min.y) * 0.5f32, (max.z - min.z) * 0.5f32);
        let mut b = AABoxClass::default();
        b.init(&center, &extent);
        b
    }

    /// `init_volume(bound_min, bound_max, &vol)`: the box grown by `MaxObjExtent`, its corner cells clamped to the grid,
    /// the outer edge incremented "due to the way we traverse the grid".
    pub fn init_volume(&self, bound_min: &Vector3, bound_max: &Vector3) -> VolumeStruct {
        // expand the box by the maximum size of any object
        let mut grid_min = *bound_min;
        grid_min.x -= self.max_obj_extent;
        grid_min.y -= self.max_obj_extent;
        grid_min.z -= self.max_obj_extent;

        let mut grid_max = *bound_max;
        grid_max.x += self.max_obj_extent;
        grid_max.y += self.max_obj_extent;
        grid_max.z += self.max_obj_extent;

        // now compute the grid coordinates of the corners of the box
        let (mut vmin, _) = self.map_point_to_cell(&grid_min);
        let (mut vmax, _) = self.map_point_to_cell(&grid_max);

        // now clamp the volume to the actual grid
        self.clamp_indices_to_grid(&mut vmin);
        self.clamp_indices_to_grid(&mut vmax);

        // increment the outer edge of the box due to the way we traverse the grid...
        vmax[0] += 1;
        vmax[1] += if plant(11338) { 0 } else { 1 };
        vmax[2] += 1;

        VolumeStruct { min: vmin, max: vmax }
    }

    /// `init_volume(line, &vol)`.
    pub fn init_volume_line(&self, line: &LineSegClass) -> VolumeStruct {
        let min_pt = Vector3::new(wwmath::min(line.get_p0().x, line.get_p1().x), wwmath::min(line.get_p0().y, line.get_p1().y), wwmath::min(line.get_p0().z, line.get_p1().z));
        let max_pt = Vector3::new(wwmath::max(line.get_p0().x, line.get_p1().x), wwmath::max(line.get_p0().y, line.get_p1().y), wwmath::max(line.get_p0().z, line.get_p1().z));
        self.init_volume(&min_pt, &max_pt)
    }

    /// `init_volume(box, &vol)`.
    pub fn init_volume_aabox(&self, box_: &AABoxClass) -> VolumeStruct {
        self.init_volume(&(box_.center - box_.extent), &(box_.center + box_.extent))
    }

    /// `init_volume(obbox, &vol)`: from its axis-aligned extent.
    pub fn init_volume_obbox(&self, box_: &OBBoxClass) -> VolumeStruct {
        let aaextent = box_.compute_axis_aligned_extent();
        self.init_volume(&(box_.center - aaextent), &(box_.center + aaextent))
    }

    /// `init_volume(frustum, &vol)`: from the frustum's bound.
    pub fn init_volume_frustum(&self, frustum: &FrustumClass) -> VolumeStruct {
        self.init_volume(frustum.get_bound_min(), frustum.get_bound_max())
    }

    /// The loop shared by the four `Collect_Objects` overloads: every cell of the volume (trivially accepted), then the
    /// no-grid list. The address walk is upstream's (`address++`, `-= delta_x; += CellCount[0]`, re-seek per k).
    fn collect_volume(&mut self, vol: &VolumeStruct, leaf: &mut dyn FnMut(&mut CullSystemClass, &Option<CullableHandle>)) {
        if !vol.is_empty() {
            let delta_x = vol.max[0] - vol.min[0];
            let mut address = self.map_indices_to_address(vol.min[0], vol.min[1], vol.min[2]);
            for k in vol.min[2]..vol.max[2] {
                for _j in vol.min[1]..vol.max[1] {
                    for _i in vol.min[0]..vol.max[0] {
                        // GRIDCULL_NODE_TRIVIALLY_ACCEPTED;
                        let head = self.cells[address as usize].clone();
                        leaf(&mut self.base, &head);
                        address += 1;
                    }
                    address -= delta_x;
                    address += self.cell_count[0];
                }
                address = self.map_indices_to_address(vol.min[0], vol.min[1], if plant(11339) { k } else { k + 1 });
            }
        }

        // objects outside or too big for the grid are always candidates
        let head = self.no_grid_list.clone();
        leaf(&mut self.base, &head);
    }

    /// `Collect_Objects(const Vector3 &)`: `init_volume(point, point)`.
    pub fn collect_objects_point(&mut self, point: &Vector3) {
        let vol = self.init_volume(point, point);
        self.collect_volume(&vol, &mut |base, head| collect_objects_in_leaf(base, head, |b| plant(11340) || b.contains_point(point)));
    }

    /// `Collect_Objects(const AABoxClass &)`.
    pub fn collect_objects_aabox(&mut self, box_: &AABoxClass) {
        let vol = self.init_volume_aabox(box_);
        self.collect_volume(&vol, &mut |base, head| collect_objects_in_leaf(base, head, |b| overlap_test_aabox_aabox(box_, b) != OverlapType::OUTSIDE));
    }

    /// `Collect_Objects(const OBBoxClass &)`.
    pub fn collect_objects_obbox(&mut self, box_: &OBBoxClass) {
        let vol = self.init_volume_obbox(box_);
        self.collect_volume(&vol, &mut |base, head| collect_objects_in_leaf(base, head, |b| overlap_test_obbox_aabox(box_, b) != OverlapType::OUTSIDE));
    }

    /// `Collect_Objects(const FrustumClass &)`.
    pub fn collect_objects_frustum(&mut self, frustum: &FrustumClass) {
        let vol = self.init_volume_frustum(frustum);
        self.collect_volume(&vol, &mut |base, head| collect_objects_in_leaf(base, head, |b| overlap_test_frustum_aabox(frustum, b) != OverlapType::OUTSIDE));
    }

    /// `Re_Partition(input_min, input_max, objdim)` (gridcull.cpp:236): all objects are unlinked into the collection, the
    /// grid is recomputed (halving the cell along the dimension that is biggest relative to `MinCellSize` until it
    /// terminates), then every object is relinked.
    pub fn re_partition(&mut self, input_min: &Vector3, input_max: &Vector3, objdim: f32) {
        // collect and unlink all objects
        self.base.reset_collection();
        self.collect_and_unlink_all();

        let mut min = *input_min;
        let mut max = *input_max;

        // "make sure the world has some size" (an axis thinner than 1 grows by MinCellSize each way)
        if max.x - min.x < 1.0f32 {
            max.x += self.min_cell_size.x;
            min.x -= self.min_cell_size.x;
        }
        if max.y - min.y < 1.0f32 {
            max.y += self.min_cell_size.y;
            min.y -= self.min_cell_size.y;
        }
        if max.z - min.z < 1.0f32 {
            max.z += self.min_cell_size.z;
            min.z -= self.min_cell_size.z;
        }

        self.origin = min;
        let world_dim = max - min;
        self.max_obj_extent = objdim;
        assert!(world_dim.x > 0.0f32);
        assert!(world_dim.y > 0.0f32);
        assert!(world_dim.z > 0.0f32);

        // start with one cell the size of the world and subdivide
        self.cell_count = [1, 1, 1];
        self.cell_dim = world_dim;

        let mut done = false;
        while !done {
            // find the dimension that is biggest relative to the minimum cell size
            let mut bigdim = 0usize;
            if if plant(11335) { self.cell_dim[1] / self.min_cell_size[1] >= self.cell_dim[bigdim] / self.min_cell_size[bigdim] } else { self.cell_dim[1] / self.min_cell_size[1] > self.cell_dim[bigdim] / self.min_cell_size[bigdim] } {
                bigdim = 1;
            }
            if self.cell_dim[2] / self.min_cell_size[2] > self.cell_dim[bigdim] / self.min_cell_size[bigdim] {
                bigdim = 2;
            }

            // split it in two if it can be split
            if self.cell_dim[bigdim] >= 2.0f32 * self.min_cell_size[bigdim] {
                self.cell_dim[bigdim] /= 2.0f32;
                self.cell_count[bigdim] *= 2;
            }

            // stop when there are too many cells, or no dimension can be split
            if if plant(11336) { self.total_cell_count() > self.termination_cell_count } else { self.total_cell_count() >= self.termination_cell_count } {
                done = true;
            }
            if (self.cell_dim[0] as f64) < 2.0 * self.min_cell_size[0] as f64 && (self.cell_dim[1] as f64) < 2.0 * self.min_cell_size[1] as f64 && (self.cell_dim[2] as f64) < 2.0 * self.min_cell_size[2] as f64 {
                done = true;
            }
        }

        self.oo_cell_dim.x = 1.0f32 / self.cell_dim.x;
        self.oo_cell_dim.y = 1.0f32 / self.cell_dim.y;
        self.oo_cell_dim.z = 1.0f32 / self.cell_dim.z;

        // allocate the cell array, all empty
        self.cells = vec![None; self.total_cell_count() as usize];

        // relink all the objects
        let mut obj = self.base.get_first_collected_object_internal();
        while let Some(o) = obj {
            self.link_object(&o);
            obj = self.base.get_next_collected_object_internal(Some(&o));
        }
    }

    /// `Collect_And_Unlink_All()`: every object in every cell and in the no-grid list into the collection.
    fn collect_and_unlink_all(&mut self) {
        self.base.reset_collection();

        for a in 0..self.cell_count[2] {
            for j in 0..self.cell_count[1] {
                for b in 0..self.cell_count[0] {
                    let (i, k) = if plant(11337) { (a, b) } else { (b, a) };
                    if plant(11337) && (i >= self.cell_count[0] || k >= self.cell_count[2]) {
                        continue;
                    }
                    let mut obj = self.cells[self.map_indices_to_address(i, j, k) as usize].clone();
                    while let Some(o) = obj {
                        let nextobj = get_next_object(&o);
                        self.unlink_object(&o);
                        self.base.add_to_collection(&o);
                        obj = nextobj;
                    }
                }
            }
        }

        let mut obj = self.no_grid_list.clone();
        while let Some(o) = obj {
            let nextobj = get_next_object(&o);
            self.unlink_object(&o);
            self.base.add_to_collection(&o);
            obj = nextobj;
        }
    }

    /// `Update_Culling(obj)`: relink when the cell the box's centre falls in has changed.
    pub fn update_culling(&mut self, obj: &CullableHandle) {
        assert!(obj.read().unwrap().get_culling_system() == Some(self.base.id));
        let (address, _) = self.map_point_to_address(&(if plant(11342) { cull_box_of(obj).extent } else { cull_box_of(obj).center }));
        let grid_address = link_of(obj, |l| l.grid_address);
        if address != grid_address || plant(11334) {
            self.unlink_object(obj);
            self.link_object_at(obj, address);
        }
    }

    /// `Load(cload)` (gridcull.cpp:290): the version chunk, then the parameters; objects are relinked.
    pub fn load(&mut self, cload: &mut ChunkLoad<'_>) -> ChunkIoResult<()> {
        cload.open_chunk()?;
        assert!(cload.cur_chunk_id()? == GRID_CHUNK_VERSION);
        let mut vb = [0u8; 4];
        cload.read_into(&mut vb)?;
        let _version = u32::from_le_bytes(vb);
        cload.close_chunk()?;

        let mut params = IOGridParametersStruct::default();
        cload.open_chunk()?;
        assert!(cload.cur_chunk_id()? == GRID_CHUNK_PARAMETERS);
        let mut pb = [0u8; 52];
        cload.read_into(&mut pb)?;
        params = IOGridParametersStruct { ..IOGridParametersStruct::from_bytes(&pb) };
        cload.close_chunk()?;

        self.collect_and_unlink_all();

        self.cell_count[0] = params.cell_count[0] as i32;
        self.cell_count[1] = params.cell_count[1] as i32;
        self.cell_count[2] = params.cell_count[2] as i32;
        self.cell_dim = Vector3::new(params.cell_dim[0], params.cell_dim[1], params.cell_dim[2]);
        self.max_obj_extent = params.max_obj_extent;
        self.min_cell_size = Vector3::new(params.min_cell_size[0], params.min_cell_size[1], params.min_cell_size[2]);
        self.origin = Vector3::new(params.origin[0], params.origin[1], params.origin[2]);

        self.oo_cell_dim.x = 1.0f32 / self.cell_dim.x;
        self.oo_cell_dim.y = 1.0f32 / self.cell_dim.y;
        self.oo_cell_dim.z = 1.0f32 / self.cell_dim.z;

        self.cells = vec![None; self.total_cell_count() as usize];

        // relink all the objects
        let mut obj = self.base.get_first_collected_object_internal();
        while let Some(o) = obj {
            self.link_object(&o);
            obj = self.base.get_next_collected_object_internal(Some(&o));
        }
        Ok(())
    }

    /// `Save(csave)`: the version chunk, then the parameters chunk.
    pub fn save(&self, csave: &mut ChunkSaveClass) {
        let version = GRID_CURRENT_VERSION;
        csave.begin_chunk(GRID_CHUNK_VERSION);
        csave.write(&version.to_le_bytes());
        csave.end_chunk();

        let mut params = IOGridParametersStruct::default();
        params.cell_count = [self.cell_count[0] as u32, self.cell_count[1] as u32, self.cell_count[2] as u32];
        params.cell_dim = [self.cell_dim.x, self.cell_dim.y, self.cell_dim.z];
        params.max_obj_extent = self.max_obj_extent;
        params.min_cell_size = [self.min_cell_size.x, self.min_cell_size.y, self.min_cell_size.z];
        params.origin = [self.origin.x, self.origin.y, self.origin.z];

        csave.begin_chunk(GRID_CHUNK_PARAMETERS);
        csave.write(&params.to_bytes());
        csave.end_chunk();
    }

    /// `Add_Object_Internal(obj)`.
    pub fn add_object_internal(&mut self, obj: &CullableHandle) {
        assert!(obj.read().unwrap().get_culling_system().is_none());
        let link = GridLinkClass::new(self.base.id);
        obj.write().unwrap().set_cull_link(Some(CullLinkClass::Grid(link)));
        self.link_object(obj);
        self.obj_count += 1;
        // obj->Add_Ref(): the cell list holds a handle.
    }

    /// `Remove_Object_Internal(obj)`.
    pub fn remove_object_internal(&mut self, obj: &CullableHandle) {
        assert!(obj.read().unwrap().get_culling_system() == Some(self.base.id));
        self.unlink_object(obj);
        obj.write().unwrap().set_culling_system(None);
        obj.write().unwrap().set_cull_link(None);
        if !plant(11341) {
            self.obj_count -= 1;
        }
        // obj->Release_Ref()
    }

    /// `link_object(obj)`: into the cell the box's centre falls in.
    fn link_object(&mut self, obj: &CullableHandle) {
        assert!(obj.read().unwrap().get_culling_system() == Some(self.base.id));
        let (address, _) = self.map_point_to_address(&cull_box_of(obj).center);
        self.link_object_at(obj, address);
    }

    /// `link_object(obj, address)`: "objects bigger than MaxObjExtent, or outside the grid, go in the no-grid list".
    fn link_object_at(&mut self, obj: &CullableHandle, address: i32) {
        assert!(obj.read().unwrap().get_culling_system() == Some(self.base.id));
        let box_ = cull_box_of(obj);

        if (if plant(11333) { box_.extent.x >= self.max_obj_extent } else { box_.extent.x > self.max_obj_extent }) || box_.extent.y > self.max_obj_extent || box_.extent.z > self.max_obj_extent || address == UNGRIDDED_ADDRESS {
            link_of_mut(obj, |l| l.grid_address = UNGRIDDED_ADDRESS);
            link_object_to_list(&mut self.no_grid_list, obj);
        } else {
            link_of_mut(obj, |l| l.grid_address = address);
            link_object_to_list(&mut self.cells[address as usize], obj);
        }
    }

    /// `unlink_object(obj)`.
    fn unlink_object(&mut self, obj: &CullableHandle) {
        assert!(obj.read().unwrap().get_culling_system() == Some(self.base.id));
        let address = link_of(obj, |l| l.grid_address);
        if address == UNGRIDDED_ADDRESS {
            unlink_object_from_list(&mut self.no_grid_list, obj);
        } else {
            unlink_object_from_list(&mut self.cells[address as usize], obj);
        }
    }
}

impl Default for GridCullSystemClass {
    fn default() -> Self {
        GridCullSystemClass::new()
    }
}

/// `link_object_to_list(head, obj)`: push on the front of a doubly linked list.
fn link_object_to_list(head: &mut Option<CullableHandle>, obj: &CullableHandle) {
    let old_head = head.take();
    link_of_mut(obj, |l| {
        l.next = old_head.clone();
        l.prev = None;
    });
    if let Some(next) = &old_head {
        link_of_mut(next, |l| l.prev = Some(obj.clone()));
    }
    *head = Some(obj.clone());
}

/// `unlink_object_from_list(head, obj)`: upstream asserts (debug) that `obj` is in the list.
fn unlink_object_from_list(head: &mut Option<CullableHandle>, obj: &CullableHandle) {
    let (prev, next) = link_of(obj, |l| (l.prev.clone(), l.next.clone()));

    if let Some(h) = head {
        if std::sync::Arc::ptr_eq(h, obj) {
            *head = next.clone();
        }
    }

    if let Some(p) = &prev {
        link_of_mut(p, |l| l.next = next.clone());
    }

    if let Some(n) = &next {
        link_of_mut(n, |l| l.prev = prev.clone());
    }

    link_of_mut(obj, |l| {
        l.prev = None;
        l.next = None;
    });
}

/// The four `collect_objects_in_leaf(volume, head)` overloads: walk the cell's list and add each object whose cull box
/// passes `test` (`GridListIterator` is the walk of `Next`).
fn collect_objects_in_leaf(base: &mut CullSystemClass, head: &Option<CullableHandle>, test: impl Fn(&AABoxClass) -> bool) {
    let mut cur = head.clone();
    while let Some(obj) = cur {
        if test(&cull_box_of(&obj)) {
            base.add_to_collection(&obj);
        }
        cur = get_next_object(&obj);
    }
}
