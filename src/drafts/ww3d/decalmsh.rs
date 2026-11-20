//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Differential coverage (#215, tests/differential_draft_decalpoly.rs, 20,000 cases, 0 mismatches): `DecalPolyClass::{reset, add_vertex, clip}` only. NOT covered:
//! `DecalMeshClass` and `MultiFixed...` (`create_decal`, `render`, `delete_decal`, `destroyed_decal_ids`, the run/range helpers): they need `MeshClass`, `MeshModelClass` and the DX8 buffers.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/decalmsh.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/decalmsh.h
//!
//! Design: upstream's decal meshes hold a `MeshClass *Parent` and a `DecalSystemClass *`. The parent mesh's data
//! the code reads (polygon/vertex/normal arrays, transform) arrives as a [`DecalParentMesh`] argument, and the
//! destroy notification (`DecalSystem->Decal_Mesh_Destroyed`) is returned by [`RigidDecalMeshClass::destroyed_decal_ids`]
//! for the owner to forward (Rust has no destructor call order to rely on). `Render` returns the vertex data and
//! draw runs instead of filling DX8 buffers.
//!
//! Upstream defects kept, flagged for the differential pass:
//! - `Delete_Decal` re-indexes with `>` (not `>=`) `VertexStartIndex`, so a poly index equal to the start index is
//!   not shifted;
//! - `Delete_Decal` releases refs with loops `for (fi = FaceStartIndex; fi < FaceCount; ...)` (bounds are counts, not
//!   an end index); refs are not modelled here so this has no effect, but the loop bound is noted;
//! - the `// TODO` degenerate-triangle check in `Create_Decal` does nothing.

use super::coltest::ObBoxClass;
use super::decalsys::DecalGeneratorClass;
use super::handles::{TextureHandle, VertexMaterialHandle};
use crate::formats::w3d::shader::ShaderClass;
use crate::logic::modules::plant;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::plane::PlaneClass;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;

/// `DecalPolyClass` (decalmsh.cpp:48): a convex polygon being clipped, with normals riding along.
#[derive(Clone, Debug, Default)]
pub struct DecalPolyClass {
    pub verts: Vec<Vector3>,
    pub vert_norms: Vec<Vector3>,
}

impl DecalPolyClass {
    /// `DecalPolyClass::Reset`.
    pub fn reset(&mut self) {
        self.verts.clear();
        self.vert_norms.clear();
    }

    /// `DecalPolyClass::Add_Vertex`.
    pub fn add_vertex(&mut self, point: &Vector3, normal: &Vector3) {
        self.verts.push(*point);
        self.vert_norms.push(*normal);
    }

    /// `PlaneClass::In_Front(point)` (plane.h:183): `Dot_Product(point, N) > D`.
    fn in_front(plane: &PlaneClass, point: &Vector3) -> bool {
        if plant(19131) { Vector3::dot_product(point, &plane.n) >= plane.d } else { Vector3::dot_product(point, &plane.n) > plane.d }
    }

    /// `Vector3::Lerp(a, b, alpha, &out)` (vector3.h): `a + (b - a) * alpha` per component.
    fn lerp(a: &Vector3, b: &Vector3, alpha: f32) -> Vector3 {
        Vector3::new(a.x + (b.x - a.x) * alpha, a.y + (b.y - a.y) * alpha, a.z + (b.z - a.z) * (if plant(19136) { 1.0 - alpha } else { alpha }))
    }

    /// `DecalPolyClass::Clip` (decalmsh.cpp:81): Sutherland-Hodgeman against one plane into `dest`.
    /// (`DISABLE_CLIPPING` is 0.)
    pub fn clip(&self, plane: &PlaneClass, dest: &mut DecalPolyClass) {
        dest.reset();
        let count = self.verts.len();
        if if plant(19132) { count < 2 } else { count <= 2 } {
            return;
        }
        let mut i = 0usize;
        let mut iprev = count - 1;
        let mut prev_point_in_front = Self::in_front(plane, &self.verts[if plant(19133) { 0 } else { iprev }]);
        for _j in 0..count {
            let cur_point_in_front = Self::in_front(plane, &self.verts[i]);
            if prev_point_in_front {
                if cur_point_in_front {
                    dest.add_vertex(&self.verts[i], &self.vert_norms[i]);
                } else {
                    // Compute_Intersection's return value is ignored; alpha is whatever it wrote (the previous
                    // value when the segment is parallel: carried as 0 here, upstream's `alpha` is uninitialised).
                    let (_, t) = plane.compute_intersection(&self.verts[iprev], &self.verts[i]);
                    let alpha = t.unwrap_or(0.0);
                    let int_point = Self::lerp(&self.verts[iprev], &self.verts[i], alpha);
                    let int_normal = if plant(19134) { Self::lerp(&self.vert_norms[i], &self.vert_norms[iprev], alpha) } else { Self::lerp(&self.vert_norms[iprev], &self.vert_norms[i], alpha) };
                    dest.add_vertex(&int_point, &int_normal);
                }
            } else if cur_point_in_front {
                let (_, t) = plane.compute_intersection(&self.verts[iprev], &self.verts[i]);
                let alpha = t.unwrap_or(0.0);
                let int_point = Self::lerp(&self.verts[iprev], &self.verts[i], alpha);
                let int_normal = Self::lerp(&self.vert_norms[iprev], &self.vert_norms[i], alpha);
                dest.add_vertex(&int_point, &int_normal);
                if !plant(19135) {
                    dest.add_vertex(&self.verts[i], &self.vert_norms[i]);
                }
            }
            prev_point_in_front = cur_point_in_front;
            iprev = i;
            i = (i + 1) % count;
        }
    }
}

/// `DecalMeshClass::DecalStruct` (decalmsh.h).
#[derive(Clone, Copy, Debug, Default)]
pub struct DecalStruct {
    pub decal_id: u32,
    pub face_start_index: i32,
    pub face_count: i32,
    pub vertex_start_index: i32,
    pub vertex_count: i32,
}

/// What `Create_Decal` reads from `MeshClass *Parent` / its `MeshModelClass`.
#[derive(Clone, Debug)]
pub struct DecalParentMesh {
    /// `Parent->Get_Transform()`.
    pub transform: Matrix3D,
    /// `model->Get_Polygon_Array()`.
    pub polys: Vec<[u32; 3]>,
    /// `model->Get_Vertex_Array()`.
    pub verts: Vec<Vector3>,
    /// `model->Get_Vertex_Normal_Array()`.
    pub vnorms: Vec<Vector3>,
    /// `DX8Wrapper::Get_Current_Caps()->Support_ZBias()`.
    pub supports_zbias: bool,
}

impl DecalParentMesh {
    /// `MeshGeometryClass::Compute_Plane(pidx, &plane)`: `PlaneClass(v0, v1, v2)` of the polygon.
    fn compute_plane(&self, pidx: usize) -> PlaneClass {
        let p = self.polys[pidx];
        PlaneClass::from_points(&self.verts[p[0] as usize], &self.verts[p[1] as usize], &self.verts[p[2] as usize])
    }
}

/// One draw run produced by `Render`.
#[derive(Clone, Debug, PartialEq)]
pub struct DecalDraw {
    /// `3 * cur_poly_index`.
    pub start_index: u32,
    pub poly_count: u32,
    /// `Polys[cur].I`.
    pub min_vertex_index: u32,
    /// `1 + Polys[next-1].K - Polys[cur].I`.
    pub vertex_count: u32,
    pub texture: Option<TextureHandle>,
    pub material: Option<VertexMaterialHandle>,
    pub shader: ShaderClass,
}

/// One vertex of `VertexFormatXYZNDUV2` as `Render` writes it (`diffuse` = 0xFFFFFFFF, second UV 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecalVertex {
    pub pos: Vector3,
    pub normal: Vector3,
    pub diffuse: u32,
    pub uv1: Vector2,
    pub uv2: Vector2,
}

fn make_vertex(pos: Vector3, normal: Vector3, uv: Vector2) -> DecalVertex {
    DecalVertex { pos, normal, diffuse: 0xFFFF_FFFF, uv1: uv, uv2: Vector2::new(0.0, 0.0) }
}

/// The `Process_Material_Run` + draw loop shared by both classes' `Render` (decalmsh.cpp:206 / 478): runs of
/// polygons with the same texture, shader and vertex material (of the first vertex). The `Set_*` calls are
/// device-only.
fn material_runs(
    polys: &[[u32; 3]],
    textures: &[Option<TextureHandle>],
    shaders: &[ShaderClass],
    vertex_materials: &[Option<VertexMaterialHandle>],
) -> Vec<DecalDraw> {
    let mut draws = Vec::new();
    let mut cur_poly_index = 0usize;
    let mut next_poly_index = 0usize;
    while next_poly_index < polys.len() {
        // Process_Material_Run(cur_poly_index)
        let start = cur_poly_index;
        let mut next = start;
        while next < polys.len()
            && textures[next] == textures[start]
            && shaders[next] == shaders[start]
            && vertex_materials[next] == vertex_materials[start]
        {
            next += 1;
        }
        next_poly_index = next;
        draws.push(DecalDraw {
            start_index: 3 * cur_poly_index as u32,
            poly_count: (next_poly_index - cur_poly_index) as u32,
            min_vertex_index: polys[cur_poly_index][0],
            vertex_count: 1 + polys[next_poly_index - 1][2] - polys[cur_poly_index][0],
            texture: textures[start],
            material: vertex_materials[polys[start][0] as usize],
            shader: shaders[start],
        });
        cur_poly_index = next_poly_index;
    }
    draws
}

/// `Delete_Range(start, count)` of `SimpleDynVecClass`.
fn delete_range<T>(v: &mut Vec<T>, start: i32, count: i32) {
    let (s, c) = (start as usize, count as usize);
    v.drain(s..s + c);
}

/// `class RigidDecalMeshClass` (decalmsh.h): decals on a rigid mesh, in the mesh's own coordinate system.
#[derive(Clone, Debug, Default)]
pub struct RigidDecalMeshClass {
    pub decals: Vec<DecalStruct>,
    pub polys: Vec<[u32; 3]>,
    pub shaders: Vec<ShaderClass>,
    pub textures: Vec<Option<TextureHandle>>,
    pub verts: Vec<Vector3>,
    pub vert_norms: Vec<Vector3>,
    pub vertex_materials: Vec<Option<VertexMaterialHandle>>,
    pub tex_coords: Vec<Vector2>,
}

impl RigidDecalMeshClass {
    pub fn new() -> Self {
        Self::default()
    }

    /// `RigidDecalMeshClass::~RigidDecalMeshClass` (decalmsh.cpp:160): the ids to pass to
    /// `DecalSystem->Decal_Mesh_Destroyed(id, this)` (ref releases are not modelled).
    pub fn destroyed_decal_ids(&self) -> Vec<u32> {
        self.decals.iter().map(|d| d.decal_id).collect()
    }

    /// `RigidDecalMeshClass::Render` (decalmsh.cpp:180), the data half: `None` when there are no decals or decals
    /// are disabled (`WW3D::Are_Decals_Enabled`); otherwise the vertex array and the draw runs.
    pub fn render(&self, decals_enabled: bool) -> Option<(Vec<DecalVertex>, Vec<DecalDraw>)> {
        if self.decals.is_empty() || !decals_enabled {
            return None;
        }
        // upstream: DX8Wrapper::Set_Transform(D3DTS_WORLD, Parent->Get_Transform()) — device-only, not translated
        let mut vertices = Vec::with_capacity(self.verts.len());
        for i in 0..self.verts.len() {
            vertices.push(make_vertex(self.verts[i], self.vert_norms[i], self.tex_coords[i]));
        }
        let draws = material_runs(&self.polys, &self.textures, &self.shaders, &self.vertex_materials);
        Some((vertices, draws))
    }

    /// `RigidDecalMeshClass::Create_Decal` (decalmsh.cpp:253). `clip_poly0/1` are upstream's file-static
    /// `_DecalPoly0/_DecalPoly1`, local here. `apt` is the list of polygon indices the decal hits. Returns whether
    /// a decal was added; `generator.add_mesh(parent)` is the caller's to do with the parent's handle when `true`.
    pub fn create_decal(
        &mut self,
        generator: &mut DecalGeneratorClass,
        localbox: &ObBoxClass,
        apt: &[u32],
        parent: &DecalParentMesh,
        generator_transform: &Matrix3D,
        generator_texture_width: &dyn Fn(usize) -> u32,
        texturing_enabled: bool,
    ) -> bool {
        // Since we can't rely on the hardware polygon offset function, physically offset the decal polygons along
        // the normal of the decal generator.
        let mut zbias_offset = Vector3::new(0.0, 0.0, 0.0);
        if !parent.supports_zbias {
            const ZBIAS_DISTANCE: f32 = 0.01f32;
            zbias_offset = generator_transform.get_z_vector();
            let invtm = parent.transform.get_orthogonal_inverse();
            zbias_offset = invtm.rotate_vector(&zbias_offset);
            zbias_offset *= ZBIAS_DISTANCE;
        }

        // If any polys were collected, add a new MeshDecalStruct
        if apt.is_empty() {
            return false;
        }

        let mut newdecal = DecalStruct {
            decal_id: generator.get_decal_id(),
            face_start_index: self.polys.len() as i32,
            face_count: 0,
            vertex_start_index: self.verts.len() as i32,
            vertex_count: 0,
        };

        let src_polys = &parent.polys;
        let src_verts = &parent.verts;
        let src_vnorms = &parent.vnorms;

        let (mat_shader, mat_texture, mat_material) = {
            let m = generator.get_material();
            (m.peek_shader(), m.get_texture(0), m.get_material())
        };

        // Set up the generator for our coordinate system
        generator.set_mesh_transform(&parent.transform, texturing_enabled, generator_texture_width);

        // Compute the clipping planes
        let b = &localbox.basis;
        let rotate = |v: [f32; 3]| -> Vector3 {
            // Matrix3x3::Rotate_Vector(basis, v): row dot v
            Vector3::new(
                b[0][0] * v[0] + b[0][1] * v[1] + b[0][2] * v[2],
                b[1][0] * v[0] + b[1][1] * v[1] + b[1][2] * v[2],
                b[2][0] * v[0] + b[2][1] * v[1] + b[2][2] * v[2],
            )
        };
        // Matrix3x3::Get_X_Vector etc. return the column.
        let col = |c: usize| Vector3::new(b[0][c], b[1][c], b[2][c]);
        let plane_set = |normal: Vector3, point: Vector3| PlaneClass { n: normal, d: Vector3::dot_product(&normal, &point) };

        let mut planes = [PlaneClass::default(); 4];
        let extent = rotate([localbox.extent.x, 0.0, 0.0]);
        let mut direction = col(0);
        planes[0] = plane_set(-direction, localbox.center + extent);
        planes[1] = plane_set(direction, localbox.center - extent);
        let extent = rotate([0.0, localbox.extent.y, 0.0]);
        direction = col(1);
        planes[2] = plane_set(-direction, localbox.center + extent);
        planes[3] = plane_set(direction, localbox.center - extent);

        // Generate the faces and per-face info
        let mut added_polys = false;
        let pdir = col(2);
        let mut decal_poly0 = DecalPolyClass::default();
        let mut decal_poly1 = DecalPolyClass::default();
        for &a in apt {
            let a = a as usize;
            // check if the polygon is backfacing
            let plane = parent.compute_plane(a);
            let dot = Vector3::dot_product(&plane.n, &pdir);
            if dot > generator.get_backface_threshhold() {
                // Copy src_polys[apt[i]] into our clip polygon
                decal_poly0.reset();
                let poly = src_polys[a];
                for j in 0..3 {
                    decal_poly0.add_vertex(&(src_verts[poly[j] as usize] + zbias_offset), &src_vnorms[poly[j] as usize]);
                }

                // Clip against the edges of the bounding box
                decal_poly0.clip(&planes[0], &mut decal_poly1);
                decal_poly1.clip(&planes[1], &mut decal_poly0);
                decal_poly0.clip(&planes[2], &mut decal_poly1);
                decal_poly1.clip(&planes[3], &mut decal_poly0);

                // Check if the clipped polygon is empty or degenerate
                if decal_poly0.verts.len() >= 3 {
                    // Extract triangles from the clipped polygon
                    let first_vert = self.verts.len() as u32;
                    for j in 1..(decal_poly0.verts.len() - 1) {
                        // Check if this triangle is degenerate: upstream has `// TODO` here.
                        added_polys = true;
                        self.polys.push([first_vert, first_vert + j as u32, first_vert + j as u32 + 1]);
                        self.shaders.push(mat_shader);
                        self.textures.push(mat_texture);
                    }

                    // Extract verts from the clipped polygon
                    for j in 0..decal_poly0.verts.len() {
                        self.verts.push(decal_poly0.verts[j]);
                        decal_poly0.vert_norms[j].normalize();
                        self.vert_norms.push(decal_poly0.vert_norms[j]);
                        self.vertex_materials.push(mat_material);

                        // Compute the uv coordinates for this vertex
                        let stq = generator.projector.compute_texture_coordinate(self.verts.last().unwrap());
                        self.tex_coords.push(Vector2::new(stq.x, stq.y));
                    }
                }
            }
        }

        if added_polys {
            newdecal.face_count = self.polys.len() as i32 - newdecal.face_start_index;
            newdecal.vertex_count = self.verts.len() as i32 - newdecal.vertex_start_index;
            self.decals.push(newdecal);
            // generator->Add_Mesh(Parent): the caller, who owns the parent's handle.
        }
        added_polys
    }

    /// `RigidDecalMeshClass::Delete_Decal` (decalmsh.cpp:388).
    pub fn delete_decal(&mut self, id: u32) -> bool {
        let decal_index = match self.decals.iter().position(|d| d.decal_id == id) {
            Some(i) => i,
            None => return false,
        };
        let decal = self.decals[decal_index];

        // Remove all geometry used by this decal
        delete_range(&mut self.polys, decal.face_start_index, decal.face_count);
        delete_range(&mut self.verts, decal.vertex_start_index, decal.vertex_count);
        delete_range(&mut self.vert_norms, decal.vertex_start_index, decal.vertex_count);
        delete_range(&mut self.tex_coords, decal.vertex_start_index, decal.vertex_count);

        // Re-index the remaining triangle vertex indices (`>`, not `>=`, as upstream)
        for poly in self.polys.iter_mut() {
            for k in 0..3 {
                if poly[k] as i32 > decal.vertex_start_index {
                    poly[k] = (poly[k] as i32 - decal.vertex_count) as u32;
                }
            }
        }

        // Remove all materials used by this decal (ref releases not modelled)
        delete_range(&mut self.shaders, decal.face_start_index, decal.face_count);
        delete_range(&mut self.textures, decal.face_start_index, decal.face_count);
        delete_range(&mut self.vertex_materials, decal.vertex_start_index, decal.vertex_count);

        // Remove MeshDecal and refresh all other decal indices
        for di in (decal_index + 1)..self.decals.len() {
            self.decals[di].face_start_index -= decal.face_count;
            self.decals[di].vertex_start_index -= decal.vertex_count;
        }
        self.decals.remove(decal_index);
        true
    }
}

/// `class SkinDecalMeshClass` (decalmsh.h): decals on a skinned mesh; vertices are the parent's deformed
/// vertices (world space), referenced by `parent_vertex_indices`.
#[derive(Clone, Debug, Default)]
pub struct SkinDecalMeshClass {
    pub decals: Vec<DecalStruct>,
    pub polys: Vec<[u32; 3]>,
    pub shaders: Vec<ShaderClass>,
    pub textures: Vec<Option<TextureHandle>>,
    pub parent_vertex_indices: Vec<u32>,
    pub vertex_materials: Vec<Option<VertexMaterialHandle>>,
    pub tex_coords: Vec<Vector2>,
}

impl SkinDecalMeshClass {
    pub fn new() -> Self {
        Self::default()
    }

    /// `SkinDecalMeshClass::~SkinDecalMeshClass`: the ids for `Decal_Mesh_Destroyed`.
    pub fn destroyed_decal_ids(&self) -> Vec<u32> {
        self.decals.iter().map(|d| d.decal_id).collect()
    }

    /// `SkinDecalMeshClass::Render` (decalmsh.cpp:488), the data half. `parent_sorted` is
    /// `model->Get_Flag(MeshModelClass::SORT)` (decals on sorted meshes are refused: upstream logs and returns);
    /// `deformed_verts/normals` are `Parent->Get_Deformed_Vertices`. Skin decals are in world space (identity world
    /// transform; `Set_Transform` is device-only).
    pub fn render(
        &self,
        decals_enabled: bool,
        parent_sorted: bool,
        deformed_verts: &[Vector3],
        deformed_normals: &[Vector3],
    ) -> Option<(Vec<DecalVertex>, Vec<DecalDraw>)> {
        if self.decals.is_empty() || !decals_enabled {
            return None;
        }
        if parent_sorted {
            // WWDEBUG_SAY(("ERROR: decals applied to a sorted mesh!\n"))
            return None;
        }
        let mut vertices = Vec::with_capacity(self.parent_vertex_indices.len());
        for i in 0..self.parent_vertex_indices.len() {
            let src_i = self.parent_vertex_indices[i] as usize;
            vertices.push(make_vertex(deformed_verts[src_i], deformed_normals[src_i], self.tex_coords[i]));
        }
        let draws = material_runs(&self.polys, &self.textures, &self.shaders, &self.vertex_materials);
        Some((vertices, draws))
    }

    /// `SkinDecalMeshClass::Create_Decal` (decalmsh.cpp:545). `world_vertex_locs` is the parent's deformed
    /// world-space vertex array (asserted non-null upstream). Returns `false` when `apt` is empty; otherwise adds
    /// one decal of `3 * apt.len()` vertices and returns `true` (caller then does `generator.add_mesh(parent)`).
    pub fn create_decal(
        &mut self,
        generator: &mut DecalGeneratorClass,
        apt: &[u32],
        parent_polys: &[[u32; 3]],
        world_vertex_locs: &[Vector3],
        texturing_enabled: bool,
        generator_texture_width: &dyn Fn(usize) -> u32,
    ) -> bool {
        if apt.is_empty() {
            return false;
        }
        let mut newdecal = DecalStruct {
            decal_id: generator.get_decal_id(),
            face_start_index: self.polys.len() as i32,
            face_count: 0,
            vertex_start_index: self.parent_vertex_indices.len() as i32,
            vertex_count: 0,
        };

        let (mat_shader, mat_texture, mat_material) = {
            let m = generator.get_material();
            (m.peek_shader(), m.get_texture(0), m.get_material())
        };

        // Set up the generator for the world coordinate system (the deformed vertices are in worldspace)
        generator.set_mesh_transform(&Matrix3D::from_translation(&Vector3::new(0.0, 0.0, 0.0)), texturing_enabled, generator_texture_width);

        // Generate the faces and per-face info (TODO upstream: take advantage of vertex sharing)
        let first_vert = self.parent_vertex_indices.len();
        for i in 0..apt.len() {
            let offset = (first_vert + i * 3) as u32;
            self.polys.push([offset, offset + 1, offset + 2]);
            self.shaders.push(mat_shader);
            self.textures.push(mat_texture);
        }

        // Copy the vertices and per-vertex info
        for i in 0..apt.len() {
            let face_index = apt[i] as usize;
            for vi in 0..3 {
                self.parent_vertex_indices.push(parent_polys[face_index][vi]);
                self.vertex_materials.push(mat_material);
                let last = *self.parent_vertex_indices.last().unwrap() as usize;
                let stq = generator.projector.compute_texture_coordinate(&world_vertex_locs[last]);
                self.tex_coords.push(Vector2::new(stq.x, stq.y));
            }
        }

        newdecal.face_count = self.polys.len() as i32 - newdecal.face_start_index;
        newdecal.vertex_count = self.parent_vertex_indices.len() as i32 - newdecal.vertex_start_index;
        self.decals.push(newdecal);
        true
    }

    /// `SkinDecalMeshClass::Delete_Decal` (decalmsh.cpp:660). Same shape as the rigid one; note `VertexMaterials` is
    /// per-vertex-of-decal and `Verts`/`VertNorms` do not exist (`ParentVertexIndices` plays their role).
    pub fn delete_decal(&mut self, id: u32) -> bool {
        let decal_index = match self.decals.iter().position(|d| d.decal_id == id) {
            Some(i) => i,
            None => return false,
        };
        let decal = self.decals[decal_index];

        delete_range(&mut self.polys, decal.face_start_index, decal.face_count);
        delete_range(&mut self.parent_vertex_indices, decal.vertex_start_index, decal.vertex_count);
        delete_range(&mut self.tex_coords, decal.vertex_start_index, decal.vertex_count);

        for poly in self.polys.iter_mut() {
            for k in 0..3 {
                if poly[k] as i32 > decal.vertex_start_index {
                    poly[k] = (poly[k] as i32 - decal.vertex_count) as u32;
                }
            }
        }

        delete_range(&mut self.shaders, decal.face_start_index, decal.face_count);
        delete_range(&mut self.textures, decal.face_start_index, decal.face_count);
        delete_range(&mut self.vertex_materials, decal.vertex_start_index, decal.vertex_count);

        for di in (decal_index + 1)..self.decals.len() {
            self.decals[di].face_start_index -= decal.face_count;
            self.decals[di].vertex_start_index -= decal.vertex_count;
        }
        self.decals.remove(decal_index);
        true
    }
}
