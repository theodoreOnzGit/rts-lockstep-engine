//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//! NOT differential-tested (#215, group sweepww3d, 2026-11-19): `PointGroupClass::Update_Arrays` is ~580 lines of twelve pointer-walking loops fed by the D3DX/DX8 device (`WW3D::Get_Render_Target_Resolution`, `DX8Wrapper::Get_Transform`, `D3DXMatrixRotationZ`), and `_Init` allocates the DX8 index buffers in the same function as the tables; a verbatim cut needs stand-ins for most of it. The default setters/getters are one-line field copies. Left for a later pass.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/pointgr.cpp
//!
//! `PointGroupClass`' default-value setters and getters and `_Shutdown`. The statics they touch are [`PointGroupDefaults`].
//! `PointGroupClass::Update_Arrays` (pointgr.cpp:1015-1595) and the `_Init` tables it needs are at the end of this file
//! (second part of the pass: [`PointGroupTables`], [`PointGroupArrays::update_arrays`]).

/// `PointGroupClass::PointModeEnum`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointModeEnum {
    Tris = 0,
    Quads = 1,
    ScreenSpace = 2,
}

/// The `Default*` statics of `PointGroupClass`.
#[derive(Clone, Debug, PartialEq)]
pub struct PointGroupDefaults {
    pub default_point_size: f32,
    pub default_point_color: [f32; 3],
    pub default_point_alpha: f32,
    pub default_point_orientation: u8,
    pub default_point_frame: u8,
    pub point_mode: PointModeEnum,
    pub billboard: bool,
    pub frame_row_column_count_log2: u8,
}

impl PointGroupDefaults {
    /// `PointGroupClass::Set_Point_Size` (pointgr.cpp:319).
    pub fn set_point_size(&mut self, size: f32) {
        self.default_point_size = size;
    }
    /// `PointGroupClass::Get_Point_Size` (pointgr.cpp:337).
    pub fn get_point_size(&self) -> f32 {
        self.default_point_size
    }
    /// `PointGroupClass::Set_Point_Color`.
    pub fn set_point_color(&mut self, color: [f32; 3]) {
        self.default_point_color = color;
    }
    /// `PointGroupClass::Get_Point_Color` (pointgr.cpp:372).
    pub fn get_point_color(&self) -> [f32; 3] {
        self.default_point_color
    }
    /// `PointGroupClass::Set_Point_Alpha`.
    pub fn set_point_alpha(&mut self, alpha: f32) {
        self.default_point_alpha = alpha;
    }
    /// `PointGroupClass::Get_Point_Alpha` (pointgr.cpp:407).
    pub fn get_point_alpha(&self) -> f32 {
        self.default_point_alpha
    }
    /// `PointGroupClass::Set_Point_Orientation` (pointgr.cpp:427).
    pub fn set_point_orientation(&mut self, orientation: u8) {
        self.default_point_orientation = orientation;
    }
    /// `PointGroupClass::Get_Point_Orientation` (pointgr.cpp:445).
    pub fn get_point_orientation(&self) -> u8 {
        self.default_point_orientation
    }
    /// `PointGroupClass::Set_Point_Frame` (pointgr.cpp:465).
    pub fn set_point_frame(&mut self, frame: u8) {
        self.default_point_frame = frame;
    }
    /// `PointGroupClass::Get_Point_Frame` (pointgr.cpp:483).
    pub fn get_point_frame(&self) -> u8 {
        self.default_point_frame
    }
    /// `PointGroupClass::Set_Point_Mode` (pointgr.cpp:501).
    pub fn set_point_mode(&mut self, mode: PointModeEnum) {
        self.point_mode = mode;
    }
    /// `PointGroupClass::Get_Point_Mode` (pointgr.cpp:519).
    pub fn get_point_mode(&self) -> PointModeEnum {
        self.point_mode
    }
    /// `PointGroupClass::Set_Billboard` (pointgr.cpp:674).
    pub fn set_billboard(&mut self, should_billboard: bool) {
        self.billboard = should_billboard;
    }
    /// `PointGroupClass::Get_Billboard` (pointgr.cpp:691).
    pub fn get_billboard(&self) -> bool {
        self.billboard
    }
    /// `PointGroupClass::Get_Frame_Row_Column_Count_Log2` (pointgr.cpp:709).
    pub fn get_frame_row_column_count_log2(&self) -> u8 {
        self.frame_row_column_count_log2
    }
    /// `PointGroupClass::Set_Frame_Row_Column_Count_Log2` (already translated; shown for its clamp): `MIN(frccl2, 4)`.
    pub fn set_frame_row_column_count_log2(&mut self, frccl2: u8) {
        self.frame_row_column_count_log2 = frccl2.min(4);
    }
}

/// `PointGroupClass::_Shutdown` (pointgr.cpp:1602): frees the five UV frame tables of each kind, releases the material,
/// the sorting / non-sorting quad and triangle index buffers and clears the working arrays. In Rust the tables and
/// buffers are owned values, so shutting down is dropping them; this clears the working vectors as upstream's `Clear()`.
pub fn shutdown(transformed_loc: &mut Vec<[f32; 3]>, vertex_loc: &mut Vec<[f32; 3]>, vertex_diffuse: &mut Vec<u32>, vertex_uv: &mut Vec<[f32; 2]>) {
    transformed_loc.clear();
    vertex_loc.clear();
    vertex_diffuse.clear();
    vertex_uv.clear();
}

// ---------------------------------------------------------------------------------------------------------------------
// PointGroupClass::_Init tables and PointGroupClass::Update_Arrays (second part of the pass)
// ---------------------------------------------------------------------------------------------------------------------

use crate::drafts::wwlib::matrix4::Matrix4x4;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;
use crate::ww::vector4::Vector4;
use crate::ww::wwmath;

/// `MAX_VB_SIZE` (pointgr.cpp:128).
pub const MAX_VB_SIZE: usize = 2048;
/// `MAX_TRI_POINTS` = `MAX_VB_SIZE/3` (682, integer division).
pub const MAX_TRI_POINTS: usize = MAX_VB_SIZE / 3;
/// `MAX_TRI_IB_SIZE` is the macro `3*MAX_TRI_POINTS`, unparenthesised: `3*MAX_VB_SIZE/3` after expansion, which is
/// `(3*2048)/3 = 2048`, NOT `3 * 682 = 2046`.
pub const MAX_TRI_IB_SIZE: usize = 3 * MAX_VB_SIZE / 3;
/// `MAX_QUAD_POINTS` = `MAX_VB_SIZE/4`.
pub const MAX_QUAD_POINTS: usize = MAX_VB_SIZE / 4;
/// `MAX_QUAD_IB_SIZE` is the macro `6*MAX_QUAD_POINTS`, i.e. `6*MAX_VB_SIZE/4 = 3072` after expansion (here equal to
/// `6 * 512`, as 2048 divides by 4).
pub const MAX_QUAD_IB_SIZE: usize = 6 * MAX_VB_SIZE / 4;

/// `_ScreenspaceVertexLocationSizeTable[2][3]` (pointgr.cpp:110): "vertex locations for screenspace mode - calculated to
/// cover exactly 1x1 and 2x2 pixels".
pub const SCREENSPACE_VERTEX_LOCATION_SIZE_TABLE: [[[f32; 3]; 3]; 2] = [
    [[0.5, 0.0, -1.0], [1.0, 1.0, -1.0], [0.0, 1.0, -1.0]],
    [[1.0, -0.5, -1.0], [2.7, 2.0, -1.0], [-0.7, 2.0, -1.0]],
];

/// The static tables `PointGroupClass::_Init` fills: the orientation-rotated vertex offsets (256 angles), the per-frame UV
/// tables for 1x1 .. 16x16 frame grids, and the index-buffer data of the four index buffers (`Tris` / `SortingTris` are
/// 0..`MAX_TRI_IB_SIZE`, `Quads` / `SortingQuads` two triangles per quad).
#[derive(Clone, Debug)]
pub struct PointGroupTables {
    /// `_TriVertexLocationOrientationTable[256][3]`.
    pub tri_vertex_location_orientation_table: Vec<[Vector3; 3]>,
    /// `_QuadVertexLocationOrientationTable[256][4]`.
    pub quad_vertex_location_orientation_table: Vec<[Vector3; 4]>,
    /// `_TriVertexUVFrameTable[5]`: table `i` holds `(1<<i)^2 * 3` UVs.
    pub tri_vertex_uv_frame_table: Vec<Vec<Vector2>>,
    /// `_QuadVertexUVFrameTable[5]`: table `i` holds `(1<<i)^2 * 4` UVs.
    pub quad_vertex_uv_frame_table: Vec<Vec<Vector2>>,
    /// The index data written to `Tris` and `SortingTris`.
    pub tri_indices: Vec<u16>,
    /// The index data written to `Quads` and `SortingQuads`.
    pub quad_indices: Vec<u16>,
}

impl PointGroupTables {
    /// `PointGroupClass::_Init()` (pointgr.cpp:1438): the data part. The four `NEW_REF(DX8IndexBufferClass / SortingIndex-
    /// BufferClass, ...)` buffers and `PointMaterial = VertexMaterialClass::Get_Preset(PRELIT_DIFFUSE)` are device-only,
    /// not translated; the index values written into them are `tri_indices` / `quad_indices`.
    pub fn init() -> Self {
        // Unrotated locations
        let tri_locs: [Vector3; 3] =
            [Vector3::new(0.0, -2.0, 0.0), Vector3::new(-1.732, 1.0, 0.0), Vector3::new(1.732, 1.0, 0.0)];
        let quad_locs: [Vector3; 4] = [
            Vector3::new(-0.5, 0.5, 0.0),
            Vector3::new(-0.5, -0.5, 0.0),
            Vector3::new(0.5, -0.5, 0.0),
            Vector3::new(0.5, 0.5, 0.0),
        ];

        // Fill vertex location orientation tables
        let mut tri_t = vec![[Vector3::new(0.0, 0.0, 0.0); 3]; 256];
        let mut quad_t = vec![[Vector3::new(0.0, 0.0, 0.0); 4]; 256];
        let mut angle: f32 = 0.0; // In radians
        let angle_step: f32 = (crate::ww::wwmath::WWMATH_PI * 2.0) / 256.0; // In radians
        for i in 0..256 {
            let c = wwmath::fast_cos(angle);
            let s = wwmath::fast_sin(angle);
            for j in 0..3 {
                tri_t[i][j].x = tri_locs[j].x * c - tri_locs[j].y * s;
                tri_t[i][j].y = tri_locs[j].x * s + tri_locs[j].y * c;
                tri_t[i][j].z = tri_locs[j].z;
            }
            for j in 0..4 {
                quad_t[i][j].x = quad_locs[j].x * c - quad_locs[j].y * s;
                quad_t[i][j].y = quad_locs[j].x * s + quad_locs[j].y * c;
                quad_t[i][j].z = quad_locs[j].z;
            }
            angle += angle_step;
        }

        // Fill frame UV tables. Unscaled / untranslated UVs
        let tri_uvs: [Vector2; 3] = [Vector2::new(0.5, 0.0), Vector2::new(0.0, 0.866), Vector2::new(1.0, 0.866)];
        let quad_uvs: [Vector2; 4] =
            [Vector2::new(0.0, 0.0), Vector2::new(0.0, 1.0), Vector2::new(1.0, 1.0), Vector2::new(1.0, 0.0)];

        let mut tri_uv_tables: Vec<Vec<Vector2>> = Vec::with_capacity(5);
        let mut quad_uv_tables: Vec<Vec<Vector2>> = Vec::with_capacity(5);
        for i in 0..5 {
            let rows: u32 = 1 << i;
            let count: u32 = rows * rows;

            let mut tri_table: Vec<Vector2> = Vec::with_capacity(count as usize * 3);
            let mut quad_table: Vec<Vector2> = Vec::with_capacity(count as usize * 4);

            let mut corner = Vector2::new(0.0, 0.0);
            let scale: f32 = 1.0 / (rows as f32);

            for _v in 0..rows {
                for _u in 0..rows {
                    tri_table.push(corner + (tri_uvs[0] * scale));
                    tri_table.push(corner + (tri_uvs[1] * scale));
                    tri_table.push(corner + (tri_uvs[2] * scale));

                    quad_table.push(corner + (quad_uvs[0] * scale));
                    quad_table.push(corner + (quad_uvs[1] * scale));
                    quad_table.push(corner + (quad_uvs[2] * scale));
                    quad_table.push(corner + (quad_uvs[3] * scale));

                    corner.x += scale;
                }
                corner.y += scale;
                corner.x = 0.0;
            }
            tri_uv_tables.push(tri_table);
            quad_uv_tables.push(quad_table);
        }

        // Fill up the IBs
        let mut tri_indices = vec![0u16; MAX_TRI_IB_SIZE];
        for i in 0..MAX_TRI_IB_SIZE {
            tri_indices[i] = i as u16;
        }

        let mut quad_indices = vec![0u16; MAX_QUAD_IB_SIZE];
        let mut vert: u16 = 0;
        let mut i = 0;
        while i < MAX_QUAD_IB_SIZE {
            quad_indices[i] = vert;
            quad_indices[i + 1] = vert + 1;
            quad_indices[i + 2] = vert + 2;

            quad_indices[i + 3] = vert + 2;
            quad_indices[i + 4] = vert + 3;
            quad_indices[i + 5] = vert;
            vert += 4;
            i += 6;
        }

        PointGroupTables {
            tri_vertex_location_orientation_table: tri_t,
            quad_vertex_location_orientation_table: quad_t,
            tri_vertex_uv_frame_table: tri_uv_tables,
            quad_vertex_uv_frame_table: quad_uv_tables,
            tri_indices,
            quad_indices,
        }
    }
}

/// The file-scope working arrays `VertexLoc`, `VertexDiffuse`, `VertexUV` (camera-space vertex locations, vertex
/// diffuse/alpha colours, vertex texture coordinates).
#[derive(Clone, Debug, Default)]
pub struct PointGroupArrays {
    pub vertex_loc: Vec<Vector3>,
    pub vertex_diffuse: Vec<Vector4>,
    pub vertex_uv: Vec<Vector2>,
}

/// The `PointGroupClass` members `Update_Arrays` reads, besides [`PointGroupDefaults`]: the viewport bounds set by
/// `Set_Arrays` (`VPXMin`, `VPYMin`, `VPXMax`, `VPYMax`).
#[derive(Clone, Copy, Debug, Default)]
pub struct PointGroupViewport {
    pub vp_x_min: f32,
    pub vp_y_min: f32,
    pub vp_x_max: f32,
    pub vp_y_max: f32,
}

/// What `Update_Arrays` takes from the device layer.
#[derive(Clone, Copy, Debug)]
pub struct PointGroupRenderEnv<'a> {
    /// `WW3D::Get_Render_Target_Resolution(xres, yres, ...)`: width and height in pixels.
    pub xres: i32,
    pub yres: i32,
    /// `DX8Wrapper::Get_Transform(D3DTS_VIEW, view)`, needed only for non-billboarded oriented quads.
    pub view: Option<&'a Matrix4x4>,
}

/// `D3DXMatrixRotationZ(&rotMat, angle)` (Windows D3DX): identity with `_11 = _22 = cos`, `_12 = sin`, `_21 = -sin`. Device
/// library, not translated upstream; this is its documented result, with the port's `sin` / `cos`.
fn d3dx_matrix_rotation_z(angle: f32) -> Matrix4x4 {
    let mut m = Matrix4x4::new(true);
    let c = wwmath::cos(angle);
    let s = wwmath::sin(angle);
    m.row[0][0] = c;
    m.row[1][1] = c;
    m.row[0][1] = s;
    m.row[1][0] = -s;
    m
}

/// `D3DX_PI` (d3dx8math.h): `3.14159265359f`.
const D3DX_PI: f32 = 3.141_592_7;

/// `GroundMultiplierX` / `GroundMultiplierY` (pointgr.cpp:121): "useful for particles that aren't aligned with the screen".
const GROUND_MULTIPLIER_X: Vector3 = Vector3 { x: 1.0, y: 0.0, z: 0.0 };
const GROUND_MULTIPLIER_Y: Vector3 = Vector3 { x: 0.0, y: 1.0, z: 0.0 };

impl PointGroupArrays {
    /// `PointGroupClass::Update_Arrays(point_loc, point_diffuse, point_size, point_orientation, point_frame, active_points,
    /// total_points, vnum, pnum)` (pointgr.cpp:1015): fills `VertexLoc`, `VertexUV` and (given a colour array)
    /// `VertexDiffuse` for the active points, in camera space. Returns `(vnum, pnum)`: the vertex and polygon counts of the
    /// active points.
    ///
    /// The vertex-location loop is chosen by (point mode, size array given?, orientation array given?): twelve cases,
    /// `TRIS_*`, `QUADS_*`, `SCREEN_*` (screen-space ignores the orientation: the `NOSIZE` pair and the `SIZE` pair share
    /// a body). Sizes and orientations come from the per-point arrays when given, else the defaults.
    ///
    /// `total_points` sizes the arrays (3 or 4 vertices per point, doubled "to prevent frequent reallocations") so they
    /// vary less often than the active count.
    pub fn update_arrays(
        &mut self,
        tables: &PointGroupTables,
        defaults: &PointGroupDefaults,
        viewport: &PointGroupViewport,
        env: &PointGroupRenderEnv<'_>,
        point_loc: &[Vector3],
        point_diffuse: Option<&[Vector4]>,
        point_size: Option<&[f32]>,
        point_orientation: Option<&[u8]>,
        point_frame: Option<&[u8]>,
        active_points: i32,
        total_points: i32,
    ) -> (i32, i32) {
        let quads = defaults.point_mode == PointModeEnum::Quads;
        let verts_per_point: i32 = if quads { 4 } else { 3 };
        let polys_per_point: i32 = if quads { 2 } else { 1 };

        // total_vnum/pnum reflect the size of the point arrays passed to the point group. These (instead of vnum/pnum,
        // which reflect the number of active points) are used for the resize logic.
        let total_vnum = verts_per_point * total_points;
        let vnum = verts_per_point * active_points;
        let pnum = polys_per_point * active_points;

        // Resize the arrays if they are too small (2x guardband to prevent frequent reallocations).
        if (self.vertex_loc.len() as i32) < total_vnum {
            let n = (total_vnum * 2) as usize;
            self.vertex_loc.resize(n, Vector3::new(0.0, 0.0, 0.0));
            self.vertex_uv.resize(n, Vector2::new(0.0, 0.0));
            self.vertex_diffuse.resize(n, Vector4::new(0.0, 0.0, 0.0, 0.0));
        }

        let active = active_points.max(0) as usize;
        let default_size = defaults.default_point_size;
        let default_orient = defaults.default_point_orientation as usize;

        // Generate the vertex locations from the point locations (note that both are in camera space). Vertex locations
        // depend on the point mode and the points' orientation and size.
        let mut vert: usize = 0;
        match defaults.point_mode {
            PointModeEnum::Tris => {
                let table = &tables.tri_vertex_location_orientation_table;
                match (point_size, point_orientation) {
                    // TRIS_NOSIZE_NOORIENT
                    (None, None) => {
                        // Setup constant vertex offsets (since size and orientation are invariants)
                        let scaled_offset = [
                            table[default_orient][0] * default_size,
                            table[default_orient][1] * default_size,
                            table[default_orient][2] * default_size,
                        ];
                        for i in 0..active {
                            self.vertex_loc[vert] = point_loc[i] + scaled_offset[0];
                            self.vertex_loc[vert + 1] = point_loc[i] + scaled_offset[1];
                            self.vertex_loc[vert + 2] = point_loc[i] + scaled_offset[2];
                            vert += 3;
                        }
                    }
                    // TRIS_SIZE_NOORIENT
                    (Some(size), None) => {
                        for i in 0..active {
                            self.vertex_loc[vert] = point_loc[i] + table[default_orient][0] * size[i];
                            self.vertex_loc[vert + 1] = point_loc[i] + table[default_orient][1] * size[i];
                            self.vertex_loc[vert + 2] = point_loc[i] + table[default_orient][2] * size[i];
                            vert += 3;
                        }
                    }
                    // TRIS_NOSIZE_ORIENT
                    (None, Some(orient)) => {
                        for i in 0..active {
                            let o = orient[i] as usize;
                            self.vertex_loc[vert] = point_loc[i] + table[o][0] * default_size;
                            self.vertex_loc[vert + 1] = point_loc[i] + table[o][1] * default_size;
                            self.vertex_loc[vert + 2] = point_loc[i] + table[o][2] * default_size;
                            vert += 3;
                        }
                    }
                    // TRIS_SIZE_ORIENT
                    (Some(size), Some(orient)) => {
                        for i in 0..active {
                            let o = orient[i] as usize;
                            self.vertex_loc[vert] = point_loc[i] + table[o][0] * size[i];
                            self.vertex_loc[vert + 1] = point_loc[i] + table[o][1] * size[i];
                            self.vertex_loc[vert + 2] = point_loc[i] + table[o][2] * size[i];
                            vert += 3;
                        }
                    }
                }
            }

            PointModeEnum::Quads => {
                let table = &tables.quad_vertex_location_orientation_table;
                match (point_size, point_orientation) {
                    // QUADS_NOSIZE_NOORIENT
                    (None, None) => {
                        let scaled_offset = [
                            table[default_orient][0] * default_size,
                            table[default_orient][1] * default_size,
                            table[default_orient][2] * default_size,
                            table[default_orient][3] * default_size,
                        ];
                        for i in 0..active {
                            self.vertex_loc[vert] = point_loc[i] + scaled_offset[0];
                            self.vertex_loc[vert + 1] = point_loc[i] + scaled_offset[1];
                            self.vertex_loc[vert + 2] = point_loc[i] + scaled_offset[2];
                            self.vertex_loc[vert + 3] = point_loc[i] + scaled_offset[3];
                            vert += 4;
                        }
                    }
                    // QUADS_SIZE_NOORIENT
                    (Some(size), None) => {
                        for i in 0..active {
                            self.vertex_loc[vert] = point_loc[i] + table[default_orient][0] * size[i];
                            self.vertex_loc[vert + 1] = point_loc[i] + table[default_orient][1] * size[i];
                            self.vertex_loc[vert + 2] = point_loc[i] + table[default_orient][2] * size[i];
                            self.vertex_loc[vert + 3] = point_loc[i] + table[default_orient][3] * size[i];
                            vert += 4;
                        }
                    }
                    // QUADS_NOSIZE_ORIENT
                    (None, Some(orient)) => {
                        for i in 0..active {
                            let o = orient[i] as usize;
                            self.vertex_loc[vert] = point_loc[i] + table[o][0] * default_size;
                            self.vertex_loc[vert + 1] = point_loc[i] + table[o][1] * default_size;
                            self.vertex_loc[vert + 2] = point_loc[i] + table[o][2] * default_size;
                            self.vertex_loc[vert + 3] = point_loc[i] + table[o][3] * default_size;
                            vert += 4;
                        }
                    }
                    // QUADS_SIZE_ORIENT
                    (Some(size), Some(orient)) => {
                        let billboard = defaults.billboard;
                        // `if (!Billboard) DX8Wrapper::Get_Transform(D3DTS_VIEW, view);`
                        let view = env.view;

                        for i in 0..active {
                            if !billboard {
                                // If we're not billboarding, then the coordinate we have is in screen space.
                                let rot_mat = d3dx_matrix_rotation_z((orient[i] as f32) / 255.0 * 2.0 * D3DX_PI);

                                let oriented_vec_x = rot_mat.mul_vector3(&GROUND_MULTIPLIER_X);
                                let oriented_vec_y = rot_mat.mul_vector3(&GROUND_MULTIPLIER_Y);

                                let p = point_loc[i];
                                let sz = size[i];
                                self.vertex_loc[vert] = Vector3::new(
                                    p.x + (oriented_vec_x.x + oriented_vec_y.x) * sz,
                                    p.y + (oriented_vec_x.y + oriented_vec_y.y) * sz,
                                    p.z,
                                );
                                self.vertex_loc[vert + 1] = Vector3::new(
                                    p.x + (oriented_vec_x.x - oriented_vec_y.x) * sz,
                                    p.y + (oriented_vec_x.y - oriented_vec_y.y) * sz,
                                    p.z,
                                );
                                self.vertex_loc[vert + 2] = Vector3::new(
                                    p.x + -(oriented_vec_x.x + oriented_vec_y.x) * sz,
                                    p.y + -(oriented_vec_x.y + oriented_vec_y.y) * sz,
                                    p.z,
                                );
                                self.vertex_loc[vert + 3] = Vector3::new(
                                    p.x + (-oriented_vec_x.x + oriented_vec_y.x) * sz,
                                    p.y + (-oriented_vec_x.y + oriented_vec_y.y) * sz,
                                    p.z,
                                );

                                // now apply the view transform so that this data is in the format expected upon the
                                // functions return.
                                if let Some(view) = view {
                                    for k in 0..4 {
                                        let result = view.mul_vector3(&self.vertex_loc[vert + k]);
                                        self.vertex_loc[vert + k] = Vector3::new(result.x, result.y, result.z);
                                    }
                                }
                            } else {
                                let o = orient[i] as usize;
                                self.vertex_loc[vert] = point_loc[i] + table[o][0] * size[i];
                                self.vertex_loc[vert + 1] = point_loc[i] + table[o][1] * size[i];
                                self.vertex_loc[vert + 2] = point_loc[i] + table[o][2] * size[i];
                                self.vertex_loc[vert + 3] = point_loc[i] + table[o][3] * size[i];
                            }
                            vert += 4;
                        }
                    }
                }
            }

            // Orientations are ignored for screensize pointgroups
            PointModeEnum::ScreenSpace => {
                // Offsets need to be scaled to the current screen resolution. First find x and y scale factors (sizes in
                // pixels need to be normalized to 2D cam viewplane of -1,-1 to 1,1)
                let x_scale = (viewport.vp_x_max - viewport.vp_x_min) / (env.xres as f32);
                let y_scale = (viewport.vp_y_max - viewport.vp_y_min) / (env.yres as f32);

                let mut scaled_locs = [[Vector3::new(0.0, 0.0, 0.0); 3]; 2];
                for i in 0..2 {
                    for j in 0..3 {
                        scaled_locs[i][j].x = SCREENSPACE_VERTEX_LOCATION_SIZE_TABLE[i][j][0] * x_scale;
                        scaled_locs[i][j].y = SCREENSPACE_VERTEX_LOCATION_SIZE_TABLE[i][j][1] * y_scale;
                        scaled_locs[i][j].z = SCREENSPACE_VERTEX_LOCATION_SIZE_TABLE[i][j][2];
                    }
                }

                match point_size {
                    // SCREEN_NOSIZE_NOORIENT, SCREEN_NOSIZE_ORIENT: add vertex offsets to point locations
                    None => {
                        let size_idx = if default_size <= 1.0 { 0 } else { 1 };
                        for i in 0..active {
                            self.vertex_loc[vert] = point_loc[i] + scaled_locs[size_idx][0];
                            self.vertex_loc[vert + 1] = point_loc[i] + scaled_locs[size_idx][1];
                            self.vertex_loc[vert + 2] = point_loc[i] + scaled_locs[size_idx][2];
                            vert += 3;
                        }
                    }
                    // SCREEN_SIZE_NOORIENT, SCREEN_SIZE_ORIENT
                    Some(size) => {
                        for i in 0..active {
                            let size_idx = if size[i] <= 1.0 { 0 } else { 1 };
                            self.vertex_loc[vert] = point_loc[i] + scaled_locs[size_idx][0];
                            self.vertex_loc[vert + 1] = point_loc[i] + scaled_locs[size_idx][1];
                            self.vertex_loc[vert + 2] = point_loc[i] + scaled_locs[size_idx][2];
                            vert += 3;
                        }
                    }
                }
            }
        }

        // Fill the UV vertex array. `frame_mask` ensures frames in range.
        let log2 = defaults.frame_row_column_count_log2 as u32;
        let frame_mask: u32 = !(0xFFFF_FFFFu32.wrapping_shl(log2 + log2));
        let per = verts_per_point as usize;
        let uv_table = if quads {
            &tables.quad_vertex_uv_frame_table[log2 as usize]
        } else {
            &tables.tri_vertex_uv_frame_table[log2 as usize]
        };
        if let Some(frames) = point_frame {
            // Fill UV array according to frame override array
            let mut vert = 0usize;
            for i in 0..active {
                let uv_idx = ((frames[i] as u32 & frame_mask) as usize) * per;
                for k in 0..per {
                    self.vertex_uv[vert] = uv_table[uv_idx + k];
                    vert += 1;
                }
            }
        } else {
            // Fill UV array according to frame state
            let base = ((defaults.default_point_frame as u32 & frame_mask) as usize) * per;
            let mut vert = 0usize;
            for _i in 0..active {
                for k in 0..per {
                    self.vertex_uv[vert] = uv_table[base + k];
                    vert += 1;
                }
            }
        }

        // If we have a point color array, fill the vertex diffuse array from it.
        let mut vert = 0usize;
        if let Some(diffuse) = point_diffuse {
            for i in 0..active {
                for j in 0..per {
                    self.vertex_diffuse[vert + j] = diffuse[i];
                }
                vert += per;
            }
        }

        (vnum, pnum)
    }
}
