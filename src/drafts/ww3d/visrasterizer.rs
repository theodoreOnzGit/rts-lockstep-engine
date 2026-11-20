//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases per part (test: tests/differential_draft_visrasterizer.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/visrasterizer.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/visrasterizer.h
//!   (VectorProcessorClass::Transform from WWMath/vp.cpp, the per-vertex `Matrix3D * Vector3` it falls back to)
//!
//! `IDBufferClass` (an ID buffer and a 1/z buffer with a triangle scan converter) and `VisRasterizerClass` (transforms and
//! clips triangles into homogeneous view space and hands them to the ID buffer), used by the visibility precalculation.
//! The camera is the [`VisCamera`] contract (`CameraClass`: view matrix, `Project_Camera_Space_Point`, the six view-space
//! frustum planes, and the frustum / box overlap test); upstream holds a ref-counted pointer, here the owner passes a
//! reference to each call that needs one and `Set_Camera`/`Get_Camera` are the owner's.
//!
//! **Findings kept as upstream has them (for the differential pass).**
//! - `IDBufferClass::Clear` sizes both memsets as `ResWidth * ResWidth * sizeof(uint32)` (not `ResWidth * ResHeight`): it
//!   clears too little for a buffer taller than wide and writes past the end for one wider than tall. Here it clears
//!   `ResWidth * ResWidth` elements capped at the buffer length (the in-bounds part of what upstream does).
//! - `VisPolyClass::Clip` ignores `Compute_Intersection`'s result, so `alpha` is stale (the previous crossing's) when the edge
//!   is parallel to the plane; it starts at 0.
//! - `Render_Non_Occluder_Scanline` takes `Max(Ceil(left->X), 1)` with an int `1`; same value as the occluder's `1.0f`.

use crate::logic::modules::plant;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::plane::PlaneClass;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// What `VisRasterizerClass` asks of `CameraClass`.
pub trait VisCamera {
    /// `Get_View_Matrix(&view_tm)`.
    fn get_view_matrix(&self) -> Matrix3D;
    /// `Project_Camera_Space_Point(in, out)`.
    fn project_camera_space_point(&self, p: &Vector3) -> Vector3;
    /// `Get_View_Space_Frustum_Planes()`: six planes.
    fn get_view_space_frustum_planes(&self) -> [PlaneClass; 6];
    /// `CollisionMath::Overlap_Test(Camera->Get_Frustum(), bounds) == CollisionMath::INSIDE`.
    fn frustum_contains_box(&self, center: &Vector3, extent: &Vector3) -> bool;
}

/// `IDBufferClass::ModeType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeType {
    OccluderMode = 0,
    NonOccluderMode,
}

/// `struct GradientsStruct`: the 1/z gradients of a triangle.
struct GradientsStruct {
    ooz: [f32; 3],
    dooz_dx: f32,
    dooz_dy: f32,
}

impl GradientsStruct {
    /// `GradientsStruct(verts)`.
    fn new(verts: &[Vector3; 3]) -> Self {
        let oodx = 1.0f32
            / (((verts[1].x - verts[2].x) * (verts[0].y - verts[2].y)) - ((verts[0].x - verts[2].x) * (verts[1].y - verts[2].y)));
        let oody = -oodx;
        let mut ooz = [0.0f32; 3];
        for i in 0..3 {
            ooz[i] = 1.0 / verts[i].z;
        }
        let dooz_dx = oodx * (((ooz[1] - ooz[2]) * (verts[0].y - verts[2].y)) - ((ooz[0] - ooz[2]) * (verts[1].y - verts[2].y)));
        let dooz_dy = oody * (((ooz[1] - ooz[2]) * (verts[0].x - verts[2].x)) - ((ooz[0] - ooz[2]) * (verts[1].x - verts[2].x)));
        GradientsStruct { ooz, dooz_dx, dooz_dy }
    }
}

/// `struct EdgeStruct`: scan converts one edge of a triangle accurately.
#[derive(Clone, Copy)]
struct EdgeStruct {
    /// fractional x coord
    x: f32,
    /// change in x per scanline
    x_step: f32,
    /// current y coord
    y: i32,
    /// number of scanlines left
    height: i32,
    /// current 1/z
    ooz: f32,
    /// change in 1/z per scanline
    ooz_step: f32,
}

impl EdgeStruct {
    /// `EdgeStruct(grad, verts, top, bottom)`.
    fn new(grad: &GradientsStruct, verts: &[Vector3; 3], top: usize, bottom: usize) -> Self {
        let y = wwmath::ceil(verts[top].y) as i32;
        let height = (wwmath::ceil(verts[bottom].y) - y as f32) as i32;
        let y_prestep = if plant(19094) { verts[top].y - y as f32 } else { y as f32 - verts[top].y };
        let real_height = verts[bottom].y - verts[top].y;
        let real_width = verts[bottom].x - verts[top].x;
        let x = ((real_width * y_prestep) / real_height) + verts[top].x;
        let x_step = real_width / real_height;
        let x_prestep = x - verts[top].x;
        let ooz = grad.ooz[top] + y_prestep * grad.dooz_dy + x_prestep * grad.dooz_dx;
        let ooz_step = x_step * grad.dooz_dx + grad.dooz_dy;
        EdgeStruct { x, x_step, y, height, ooz, ooz_step }
    }

    /// `Step()`: one scanline down; returns the height left.
    fn step(&mut self) -> i32 {
        self.x += self.x_step;
        self.y += 1;
        self.height -= 1;
        self.ooz += self.ooz_step;
        self.height
    }
}

/// `class IDBufferClass`: the ID buffer and the Z buffer (a 1/z buffer), the low level rasterization code, and stats on how many
/// pixels were drawn.
#[derive(Clone, Debug)]
pub struct IDBufferClass {
    pub backface_id: u32,
    pub frontface_id: u32,
    cur_id: u32,
    pub pixel_counter: i32,
    pub render_mode: ModeType,
    pub two_sided_rendering_enabled: bool,
    res_width: i32,
    res_height: i32,
    id_buffer: Vec<u32>,
    /// "actually a 1/z buffer..."
    z_buffer: Vec<f32>,
}

impl IDBufferClass {
    /// `IDBufferClass::IDBufferClass()`.
    pub fn new() -> Self {
        IDBufferClass {
            backface_id: 0,
            frontface_id: 0,
            cur_id: 0,
            pixel_counter: 0,
            render_mode: ModeType::OccluderMode,
            two_sided_rendering_enabled: false,
            res_width: 0,
            res_height: 0,
            id_buffer: Vec::new(),
            z_buffer: Vec::new(),
        }
    }

    /// `Set_Resolution(w, h)`: reallocates the buffers when the size changes.
    pub fn set_resolution(&mut self, w: i32, h: i32) {
        if w == self.res_width && h == self.res_height {
            return;
        }
        self.res_width = w;
        self.res_height = h;
        self.allocate_buffers();
    }

    /// `Get_Resolution(&w, &h)`.
    pub fn get_resolution(&self) -> (i32, i32) {
        (self.res_width, self.res_height)
    }

    /// `Reset()`: frees the buffers, zeroes the pixel counter.
    fn reset(&mut self) {
        self.id_buffer = Vec::new();
        self.z_buffer = Vec::new();
        self.pixel_counter = 0;
    }

    /// `Allocate_Buffers()`: `ResWidth * ResHeight` entries each (uninitialised upstream; zeroed here).
    fn allocate_buffers(&mut self) {
        self.reset();
        let bufsize = self.res_width * self.res_height;
        if bufsize > 0 {
            self.id_buffer = vec![0u32; bufsize as usize];
            self.z_buffer = vec![0.0f32; bufsize as usize];
        }
    }

    /// `Clear()` (see the file header for the `ResWidth * ResWidth` size).
    pub fn clear(&mut self) {
        if self.res_width > 0 && self.res_height > 0 {
            debug_assert!(!self.id_buffer.is_empty());
            debug_assert!(!self.z_buffer.is_empty());
            let count = ((self.res_width as i64 * (if plant(19098) { self.res_height } else { self.res_width }) as i64) as usize).min(self.id_buffer.len());
            for v in self.id_buffer[..count].iter_mut() {
                *v = 0;
            }
            for v in self.z_buffer[..count].iter_mut() {
                *v = 0.0;
            }
        }
    }

    /// `Pixel_Coords_To_Address(x, y)`.
    fn pixel_coords_to_address(&self, x: i32, y: i32) -> i32 {
        y * self.res_width + x
    }

    /// `Get_Pixel_Row(y, min_x, max_x)`: the ids from `(min_x, y)` to the end of the buffer.
    pub fn get_pixel_row(&self, y: i32, min_x: i32, max_x: i32) -> &[u32] {
        debug_assert!(y >= 0);
        debug_assert!(y < self.res_height);
        debug_assert!(min_x >= 0);
        debug_assert!(max_x <= self.res_width);
        let addr = self.pixel_coords_to_address(min_x, y);
        &self.id_buffer[addr as usize..]
    }

    /// `Is_Backfacing(p0, p1, p2)`: the sign of the screen-space winding.
    fn is_backfacing(p0: &Vector3, p1: &Vector3, p2: &Vector3) -> bool {
        let x1 = p1.x - p0.x;
        let y1 = p1.y - p0.y;
        let x2 = p2.x - p0.x;
        let y2 = p2.y - if plant(19091) { p1.y } else { p0.y };
        let r = x1 * y2 - x2 * y1;
        r < 0.0
    }

    /// `Render_Triangle(p0, p1, p2)`: scan converts a projected triangle (coordinates in -1..+1). True when any pixel passed
    /// the depth test.
    pub fn render_triangle(&mut self, p0: &Vector3, p1: &Vector3, p2: &Vector3) -> bool {
        if self.z_buffer.is_empty() || self.id_buffer.is_empty() {
            return false;
        }
        let mut pixels_passed = 0;
        let is_backfacing = Self::is_backfacing(p0, p1, p2);
        if is_backfacing && !self.two_sided_rendering_enabled {
            if self.render_mode == ModeType::NonOccluderMode {
                return false;
            }
            self.cur_id = self.backface_id;
        } else {
            self.cur_id = self.frontface_id;
        }

        // Transform the coordinates to device coords. All coordinates come in with the range -1 -> +1
        let rw = self.res_width as f32;
        let rh = self.res_height as f32;
        let mut points = [Vector3::new(0.0, 0.0, 0.0); 3];
        points[0].x = 0.5 * (p0.x + 1.0) * rw;
        points[1].x = 0.5 * (p1.x + 1.0) * rw;
        points[2].x = 0.5 * (p2.x + 1.0) * rw;
        points[0].y = 0.5 * (1.0 - p0.y) * rh;
        points[1].y = 0.5 * (1.0 - p1.y) * rh;
        points[2].y = 0.5 * (1.0 - p2.y) * rh;
        points[0].z = 0.5 * (p0.z + if plant(19092) { 1.0 } else { 1.001 }) * 1000.0;
        points[1].z = 0.5 * (p1.z + 1.001) * 1000.0;
        points[2].z = 0.5 * (p2.z + 1.001) * 1000.0;

        // Sort points based on Y
        let y0 = points[0].y;
        let y1 = points[1].y;
        let y2 = points[2].y;
        let (top, middle, bottom, middle_for_compare, bottom_for_compare): (usize, usize, usize, i32, i32);
        if y0 < y1 {
            if y2 < y0 {
                top = 2;
                middle = 0;
                bottom = 1;
                middle_for_compare = 0;
                bottom_for_compare = 1;
            } else {
                top = 0;
                if y1 < y2 {
                    middle = 1;
                    bottom = 2;
                    middle_for_compare = 1;
                    bottom_for_compare = 2;
                } else {
                    middle = 2;
                    bottom = 1;
                    middle_for_compare = 2;
                    bottom_for_compare = 1;
                }
            }
        } else if y2 < y1 {
            top = 2;
            middle = 1;
            bottom = 0;
            middle_for_compare = 1;
            bottom_for_compare = 0;
        } else {
            top = 1;
            if y0 < y2 {
                middle = 0;
                bottom = 2;
                middle_for_compare = 3;
                bottom_for_compare = 2;
            } else {
                middle = 2;
                bottom = 0;
                middle_for_compare = 2;
                bottom_for_compare = if plant(19093) { 0 } else { 3 };
            }
        }

        // Compute the gradients and set up the edge structures
        let grads = GradientsStruct::new(&points);
        let mut top_to_bottom_edge = EdgeStruct::new(&grads, &points, top, bottom);
        let mut top_to_middle_edge = EdgeStruct::new(&grads, &points, top, middle);
        let mut middle_to_bottom_edge = EdgeStruct::new(&grads, &points, middle, bottom);

        // "1 ^ is_backfacing" / "0 ^ is_backfacing" on an int and a bool
        let middle_is_left = if bottom_for_compare > middle_for_compare { !is_backfacing } else { is_backfacing };

        // Fill scanlines
        let occluder = self.render_mode == ModeType::OccluderMode;
        let mut height = top_to_middle_edge.height;
        while {
            let h = height;
            height -= 1;
            h != 0
        } {
            let (left, right) = if middle_is_left { (&mut top_to_middle_edge, &mut top_to_bottom_edge) } else { (&mut top_to_bottom_edge, &mut top_to_middle_edge) };
            pixels_passed += if occluder { self.render_occluder_scanline(&grads, left, right) } else { self.render_non_occluder_scanline(&grads, left, right) };
            left.step();
            right.step();
        }

        height = middle_to_bottom_edge.height;
        while {
            let h = height;
            height -= 1;
            h != 0
        } {
            let (left, right) = if middle_is_left { (&mut middle_to_bottom_edge, &mut top_to_bottom_edge) } else { (&mut top_to_bottom_edge, &mut middle_to_bottom_edge) };
            pixels_passed += if occluder { self.render_occluder_scanline(&grads, left, right) } else { self.render_non_occluder_scanline(&grads, left, right) };
            left.step();
            right.step();
        }
        pixels_passed > 0
    }

    /// `Render_Occluder_Scanline(grads, left, right)`: writes the id where the 1/z test passes (backfaces: strictly
    /// greater; front faces: greater or equal). Returns the pixels written.
    fn render_occluder_scanline(&mut self, grads: &GradientsStruct, left: &EdgeStruct, right: &EdgeStruct) -> i32 {
        if left.y < 1 || left.y >= self.res_height {
            return 0;
        }
        let xstart = wwmath::float_to_long(wwmath::max(wwmath::ceil(left.x), if plant(19097) { 0.0 } else { 1.0 }));
        let mut width = wwmath::float_to_long(wwmath::ceil(right.x)) - xstart;
        if xstart + width > self.res_width {
            width = self.res_width - xstart;
        }
        let xprestep = xstart as f32 - left.x;
        let mut address = self.pixel_coords_to_address(xstart, left.y);
        let mut ooz = left.ooz + xprestep * grads.dooz_dx;
        let mut pixel_counter = 0;

        // Two separate loops, backfaces only render when LESS THAN
        if self.cur_id == self.backface_id {
            while {
                let w = width;
                width -= 1;
                w > 0
            } {
                if if plant(19096) { ooz >= self.z_buffer[address as usize] } else { ooz > self.z_buffer[address as usize] } {
                    self.id_buffer[address as usize] = self.cur_id;
                    self.z_buffer[address as usize] = ooz;
                    pixel_counter += 1;
                }
                ooz += grads.dooz_dx;
                address += 1;
            }
        } else {
            // Front faces render when LESS THAN OR EQUAL TO
            while {
                let w = width;
                width -= 1;
                w > 0
            } {
                if if plant(19095) { ooz > self.z_buffer[address as usize] } else { ooz >= self.z_buffer[address as usize] } {
                    self.id_buffer[address as usize] = self.cur_id;
                    self.z_buffer[address as usize] = ooz;
                    pixel_counter += 1;
                }
                ooz += grads.dooz_dx;
                address += 1;
            }
        }
        self.pixel_counter += pixel_counter;
        pixel_counter
    }

    /// `Render_Non_Occluder_Scanline(grads, left, right)`: returns 1 (and counts one pixel) at the first pixel that would
    /// pass, writing nothing.
    fn render_non_occluder_scanline(&mut self, grads: &GradientsStruct, left: &EdgeStruct, right: &EdgeStruct) -> i32 {
        if left.y < 1 || left.y >= self.res_height {
            return 0;
        }
        let xstart = wwmath::float_to_long(wwmath::max(wwmath::ceil(left.x), 1.0));
        let mut width = wwmath::float_to_long(wwmath::ceil(right.x)) - xstart;
        if xstart + width > self.res_width {
            width = self.res_width - xstart;
        }
        let xprestep = xstart as f32 - left.x;
        let mut address = self.pixel_coords_to_address(xstart, left.y);
        let mut ooz = left.ooz + xprestep * grads.dooz_dx;
        while {
            let w = width;
            width -= 1;
            w > 0
        } {
            if ooz >= self.z_buffer[address as usize] {
                self.pixel_counter += 1;
                return 1;
            }
            ooz += grads.dooz_dx;
            address += 1;
        }
        0
    }
}

impl Default for IDBufferClass {
    fn default() -> Self {
        Self::new()
    }
}

/// `class VisPolyClass`: a polygon being clipped.
#[derive(Clone, Debug, Default)]
struct VisPolyClass {
    verts: Vec<Vector3>,
}

impl VisPolyClass {
    /// `Reset()`: `Verts.Delete_All(false)`.
    fn reset(&mut self) {
        self.verts.clear();
    }

    /// `Add_Vertex(point)`.
    fn add_vertex(&mut self, point: &Vector3) {
        self.verts.push(*point);
    }

    /// `Clip(plane, dest)`: the part of the polygon behind the (outward-normal) plane, into `dest`; fewer than three
    /// vertices clip to nothing.
    fn clip(&self, plane: &PlaneClass, dest: &mut VisPolyClass) {
        dest.reset();

        let mut i = 0usize;
        let vcount = self.verts.len();
        if vcount <= 2 {
            return;
        }
        let mut iprev = vcount - 1;
        // alpha is stale (the previous crossing's) when an edge is parallel to the plane; see the file header
        let mut alpha = 0.0f32;
        let in_front = |p: &Vector3| Vector3::dot_product(p, &plane.n) > plane.d;

        // perform clipping
        let mut prev_point_in_front = !in_front(&self.verts[iprev]); // Note, plane normal is outward so we invert this test
        for _ in 0..vcount {
            let cur_point_in_front = !in_front(&self.verts[i]); // Note, plane normal is out so we invert this test
            if prev_point_in_front {
                if cur_point_in_front {
                    // Previous vertex was in front of plane and this vertex is in front of the plane so we emit this vertex.
                    dest.add_vertex(&self.verts[i]);
                } else {
                    // Previous vert was in front, this vert is behind, compute the intersection and emit the point.
                    if let (_, Some(t)) = plane.compute_intersection(&self.verts[iprev], &self.verts[i]) {
                        alpha = t;
                    }
                    let int_point = Vector3::lerp(&self.verts[iprev], &self.verts[i], alpha);
                    dest.add_vertex(&int_point);
                }
            } else if cur_point_in_front {
                // segment is going from the back halfspace to the front halfspace compute the intersection and emit it,
                // then continue the edge into the front halfspace and emit the end point.
                if let (_, Some(t)) = plane.compute_intersection(&self.verts[iprev], &self.verts[i]) {
                    alpha = t;
                }
                let int_point = Vector3::lerp(&self.verts[iprev], &self.verts[i], alpha);
                dest.add_vertex(&int_point);
                if !plant(19099) {
                    dest.add_vertex(&self.verts[i]);
                }
            }
            prev_point_in_front = cur_point_in_front;
            iprev = i;
            i += 1;
            if i >= vcount {
                i = 0;
            }
        }
    }
}

/// `class VisRasterizerClass`.
#[derive(Clone, Debug)]
pub struct VisRasterizerClass {
    /// `ModelTransform` ("AKA World Transform").
    pub model_transform: Matrix3D,
    /// `MVTransform`.
    mv_transform: Matrix3D,
    /// `IDBuffer`.
    pub id_buffer: IDBufferClass,
    /// `TempVertexBuffer`.
    temp_vertex_buffer: Vec<Vector3>,
}

impl VisRasterizerClass {
    /// `VisRasterizerClass::VisRasterizerClass()`: identity transforms.
    pub fn new() -> Self {
        let mut id = Matrix3D::default();
        id.make_identity();
        VisRasterizerClass { model_transform: id, mv_transform: id, id_buffer: IDBufferClass::new(), temp_vertex_buffer: Vec::new() }
    }

    /// `Set_Resolution(width, height)`.
    pub fn set_resolution(&mut self, width: i32, height: i32) {
        self.id_buffer.set_resolution(width, height);
    }

    /// `Get_Resolution(&w, &h)`.
    pub fn get_resolution(&self) -> (i32, i32) {
        self.id_buffer.get_resolution()
    }

    /// `Set_Model_Transform(model)`: stores it and updates the model-view transform with `camera` (`Set_Camera(camera)` is
    /// the same update).
    pub fn set_model_transform<C: VisCamera>(&mut self, model: &Matrix3D, camera: Option<&C>) {
        self.model_transform = *model;
        self.update_mv_transform(camera);
    }

    /// `Get_Model_Transform()`.
    pub fn get_model_transform(&self) -> &Matrix3D {
        &self.model_transform
    }

    /// `Update_MV_Transform()`: `MVTransform = view_tm * ModelTransform` (`view_tm` identity without a camera).
    pub fn update_mv_transform<C: VisCamera>(&mut self, camera: Option<&C>) {
        let mut view_tm = Matrix3D::default();
        view_tm.make_identity();
        if let Some(c) = camera {
            view_tm = c.get_view_matrix();
        }
        self.mv_transform.mul(&view_tm, &self.model_transform);
    }

    /// `Get_MV_Transform()`: "the user can and does mess with the camera directly!", so it re-updates first.
    fn get_mv_transform<C: VisCamera>(&mut self, camera: Option<&C>) -> Matrix3D {
        self.update_mv_transform(camera);
        self.mv_transform
    }

    /// `Render_Triangles(verts, vcount, tris, tcount, bounds)`: no clipping when `bounds` lies wholly inside the camera frustum.
    /// `bounds` is `(center, extent)` of the `AABoxClass`; `tris` are vertex index triples. The camera is required (upstream
    /// dereferences `Camera` unchecked).
    pub fn render_triangles<C: VisCamera>(&mut self, camera: &C, verts: &[Vector3], vcount: usize, tris: &[[u32; 3]], tcount: usize, bounds: (&Vector3, &Vector3)) -> bool {
        debug_assert!(vcount > 0);
        debug_assert!(tcount > 0);
        // if the user supplied bounds, check if we need to clip
        if camera.frustum_contains_box(bounds.0, bounds.1) {
            self.render_triangles_no_clip(camera, verts, vcount, tris, tcount)
        } else {
            self.render_triangles_clip(camera, verts, vcount, tris, tcount)
        }
    }

    /// `Render_Triangles_No_Clip`: transform into homogeneous view space, project, and pass the triangles to the ID buffer.
    fn render_triangles_no_clip<C: VisCamera>(&mut self, camera: &C, verts: &[Vector3], vcount: usize, tris: &[[u32; 3]], tcount: usize) -> bool {
        let mut pixel_passed = false;

        // 1. transform verts into homogeneous view space and project
        let tm = self.get_mv_transform(Some(camera));
        self.temp_vertex_buffer.resize(vcount, Vector3::new(0.0, 0.0, 0.0)); // Uninitialised_Grow
        // VectorProcessorClass::Transform(tverts, verts, tm, vcount)
        tm.mul_vector3_array(&verts[..vcount], &mut self.temp_vertex_buffer[..vcount]);
        for i in 0..vcount {
            let p = self.temp_vertex_buffer[i];
            self.temp_vertex_buffer[i] = camera.project_camera_space_point(&p);
        }

        // 2. Pass triangles on to the ID buffer for scan conversion
        for tri_index in 0..tcount {
            let tri = tris[tri_index];
            let (a, b, c) = (self.temp_vertex_buffer[tri[0] as usize], self.temp_vertex_buffer[tri[1] as usize], self.temp_vertex_buffer[tri[2] as usize]);
            pixel_passed |= self.id_buffer.render_triangle(&a, &b, &c);
            if pixel_passed && self.id_buffer.render_mode == ModeType::NonOccluderMode {
                return true;
            }
        }
        pixel_passed
    }

    /// `Render_Triangles_Clip`: transform into view space, clip each triangle against the six frustum planes, project the
    /// survivors, and pass the resulting triangle fan to the ID buffer.
    fn render_triangles_clip<C: VisCamera>(&mut self, camera: &C, verts: &[Vector3], vcount: usize, tris: &[[u32; 3]], tcount: usize) -> bool {
        let mut pixel_passed = false;

        // 1. transform triangles into homogeneous view space
        let tm = self.get_mv_transform(Some(camera));
        self.temp_vertex_buffer.resize(vcount, Vector3::new(0.0, 0.0, 0.0));
        tm.mul_vector3_array(&verts[..vcount], &mut self.temp_vertex_buffer[..vcount]);

        // 2. get the frustum clipping planes
        let planes = camera.get_view_space_frustum_planes();

        // 3. Clip triangles to the view volume and pass on to the ID buffer for scan conversion
        let mut vis_poly0 = VisPolyClass::default();
        let mut vis_poly1 = VisPolyClass::default();
        for tri_index in 0..tcount {
            // Copy triangle data into the vis clipping structure
            vis_poly0.reset();
            vis_poly0.add_vertex(&self.temp_vertex_buffer[tris[tri_index][0] as usize]);
            vis_poly0.add_vertex(&self.temp_vertex_buffer[tris[tri_index][1] as usize]);
            vis_poly0.add_vertex(&self.temp_vertex_buffer[tris[tri_index][2] as usize]);

            // Clip against the view frustum
            vis_poly0.clip(&planes[0], &mut vis_poly1);
            vis_poly1.clip(&planes[1], &mut vis_poly0);
            vis_poly0.clip(&planes[2], &mut vis_poly1);
            vis_poly1.clip(&planes[3], &mut vis_poly0);
            vis_poly0.clip(&planes[4], &mut vis_poly1);
            vis_poly1.clip(&planes[5], &mut vis_poly0);

            // Project the vertices
            let final_vcount = vis_poly0.verts.len();
            if final_vcount >= 3 {
                for i in 0..final_vcount {
                    let p = vis_poly0.verts[i];
                    vis_poly0.verts[i] = camera.project_camera_space_point(&p);
                }

                // Pass the resulting triangle fan to the IDBuffer
                let mut i = if plant(19100) { 2 } else { 1 };
                while i < final_vcount - 1 {
                    let (a, b, c) = (vis_poly0.verts[0], vis_poly0.verts[i], vis_poly0.verts[i + 1]);
                    pixel_passed |= self.id_buffer.render_triangle(&a, &b, &c);
                    if pixel_passed && self.id_buffer.render_mode == ModeType::NonOccluderMode {
                        return true;
                    }
                    i += 1;
                }
            }
        }
        pixel_passed
    }
}

impl Default for VisRasterizerClass {
    fn default() -> Self {
        Self::new()
    }
}
