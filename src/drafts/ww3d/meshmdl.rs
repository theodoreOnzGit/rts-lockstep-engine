//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshmdl.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshmdl.h (`GapFillerClass`)
//!
//! Compared with `crate::formats::w3d::meshmdl` (the verified port: `Load_W3D`'s `install_materials` /
//! `post_process`, polygon groups, skin deformation). Already in the port, so skipped: the constructor's setup of
//! the default material description, `Post_Load_Process`, `Peek_Material/Texture`, `Get_Shader`, `Get_Flag`.
//! Not in the port, drafted here over `MeshModel`: `Replace_Texture`, `Replace_VertexMaterial`, `Shadow_Render`,
//! `Needs_Vertex_Normals`, `Reset`, the `Make_*_Unique` family (no-ops: `MeshModel` owns its arrays by value, there
//! is no sharing to undo), `Init_For_NPatch_Rendering` and `GapFillerClass`.
//! Not translated (reasons): `operator=`/copy constructor (`Clone`), `Register_For_Rendering` /
//! `Peek_FVF_Category_Container` / the `Change_Polygon_Renderer_*` calls (the DX8 mesh renderer's registry; the
//! port draws from `polygon_groups`), `Enable_Alternate_Material_Description` and the alternate description
//! (the port does not install it, see its module docs), `Process_Texture_Reduction` (commented out upstream).
//! Textures and vertex materials are the port's indices into `MeshModel::textures` / `vertex_materials`.

use crate::formats::w3d::meshmdl::MeshModel;
use crate::formats::w3d::shader::ShaderClass;
use crate::formats::w3d::W3dResult;
use super::bwrender::BWRenderClass;
use crate::ww::vector2::Vector2;
use crate::ww::vector4::Vector4;
use std::collections::BTreeMap;

/// `MAX_TEX_STAGES` (meshmatdesc.h).
const MAX_TEX_STAGES: usize = 2;

/// `MeshGeometryClass::PRELIT_MASK` (meshgeometry.h): `PRELIT_VERTEX | PRELIT_LIGHTMAP_MULTI_PASS |
/// PRELIT_LIGHTMAP_MULTI_TEXTURE`.
pub const PRELIT_MASK: u32 = 0x0000_0F00;

/// `MeshModelClass::Replace_Texture(texture, new_texture)` (meshmdl.cpp:207): every use of `texture` (per polygon or
/// single) in every pass and stage becomes `new_texture`. The `fvf_category->Change_Polygon_Renderer_Texture` call
/// is the DX8 renderer's.
pub fn replace_texture(model: &mut MeshModel, texture: usize, new_texture: usize) {
    for stage in 0..MAX_TEX_STAGES {
        for pass in 0..model.pass_count {
            let p = &mut model.passes[pass];
            if let Some(arr) = &mut p.texture_array[stage] {
                for slot in arr.iter_mut() {
                    if *slot == Some(texture) {
                        *slot = Some(new_texture);
                    }
                }
            } else if p.texture[stage] == Some(texture) {
                p.texture[stage] = Some(new_texture);
            }
        }
    }
}

/// `MeshModelClass::Replace_VertexMaterial(vmat, new_vmat)` (meshmdl.cpp:235).
pub fn replace_vertex_material(model: &mut MeshModel, vmat: usize, new_vmat: usize) {
    for pass in 0..model.pass_count {
        let p = &mut model.passes[pass];
        if let Some(arr) = &mut p.material_array {
            for slot in arr.iter_mut() {
                if *slot == Some(vmat) {
                    *slot = Some(new_vmat);
                }
            }
        } else if p.material == Some(vmat) {
            p.material = Some(new_vmat);
        }
    }
}

/// `MeshModelClass::Needs_Vertex_Normals` (meshmdl.cpp:361). `do_mappers_need_normals` is
/// `CurMatDesc->Do_Mappers_Need_Normals()` (meshmatdesc.cpp).
pub fn needs_vertex_normals(model: &MeshModel, do_mappers_need_normals: bool) -> bool {
    if model.flags & PRELIT_MASK == 0 {
        return true;
    }
    do_mappers_need_normals
}

/// `MeshModelClass::Reset(polycount, vertcount, passcount)` (meshmdl.cpp:157): the geometry and material
/// description are emptied to the given counts. (`MeshGeometryClass::Reset_Geometry`, `MeshMatDescClass::Reset`
/// are outside this file; the arrays are reallocated empty here.)
pub fn reset(model: &mut MeshModel, polycount: usize, vertcount: usize, passcount: usize) {
    model.vertices = vec![[0.0; 3]; vertcount];
    model.normals = vec![[0.0; 3]; vertcount];
    model.polys = vec![[0; 3]; polycount];
    model.bone_links = None;
    model.pass_count = passcount;
    model.passes.truncate(passcount);
    for p in model.passes.iter_mut() {
        p.material = None;
        p.material_array = None;
        p.shader_array = None;
        p.texture = [None; MAX_TEX_STAGES];
        p.texture_array = [None, None];
    }
    model.vertex_materials.clear();
    model.textures.clear();
    model.has_alternate = false;
}

/// `MeshModelClass::Shadow_Render` (meshmdl.cpp:274): rasterises the mesh's screen-space projection into the
/// shadow renderer. `screenspace_vertices` is `get_deformed_screenspace_vertices(..)` (meshgeometry.cpp, not
/// translated here). Each vertex becomes `Vector2(x, -y)`; the polygon indices go in as `unsigned long`s.
pub fn shadow_render(model: &MeshModel, bw_renderer: Option<&mut BWRenderClass>, screenspace_vertices: &[Vector4]) {
    if let Some(bw) = bw_renderer {
        let mut tvec: Vec<Vector2> = Vec::with_capacity(model.vertex_count());
        for v in screenspace_vertices.iter().take(model.vertex_count()) {
            tvec.push(Vector2::new(v.x, -v.y));
        }
        bw.set_vertex_locations(tvec);
        let mut idx: Vec<u32> = Vec::with_capacity(model.poly_count() * 3);
        for p in &model.polys {
            idx.extend_from_slice(&[p[0] as u32, p[1] as u32, p[2] as u32]);
        }
        bw.render_triangles(&idx);
    }
}

/// `GapFillerClass` (meshmdl.h/meshmdl.cpp): extra polygons that close the cracks N-patch tessellation opens at hard
/// edges, with the per-polygon texture / material / shader of the polygon they patch.
#[derive(Clone, Debug, Default)]
pub struct GapFillerClass {
    pub polygon_array: Vec<[u32; 3]>,
    /// `TextureArray[pass][stage]`: `Some` when the model has a texture array for that pass and stage.
    pub texture_array: Vec<[Option<Vec<Option<usize>>>; MAX_TEX_STAGES]>,
    pub material_array: Vec<Option<Vec<Option<usize>>>>,
    pub shader_array: Vec<Option<Vec<ShaderClass>>>,
    array_size: usize,
}

impl GapFillerClass {
    /// `GapFillerClass::GapFillerClass(mmc)` (meshmdl.cpp:437): room for 6 polygons per model polygon ("each side of
    /// each triangle can have 2 polygons added, in the worst case"); arrays exist where the model's do.
    pub fn new(model: &MeshModel) -> Self {
        let array_size = model.poly_count() * 6;
        let mut g = GapFillerClass { array_size, ..Default::default() };
        for pass in 0..model.pass_count {
            let p = &model.passes[pass];
            g.texture_array.push([
                p.texture_array[0].as_ref().map(|_| Vec::new()),
                p.texture_array[1].as_ref().map(|_| Vec::new()),
            ]);
            g.material_array.push(p.material_array.as_ref().map(|_| Vec::new()));
            g.shader_array.push(p.shader_array.as_ref().map(|_| Vec::new()));
        }
        g
    }

    pub fn get_polygon_count(&self) -> usize {
        self.polygon_array.len()
    }

    /// `GapFillerClass::Add_Polygon(polygon_index, vidx1, vidx2, vidx3)` (meshmdl.cpp:560): the new polygon takes
    /// the shader, the material of the patched polygon's FIRST vertex, and the textures of the patched polygon.
    pub fn add_polygon(&mut self, model: &MeshModel, polygon_index: usize, vidx1: u32, vidx2: u32, vidx3: u32) -> W3dResult<()> {
        debug_assert!(self.polygon_array.len() < self.array_size);
        debug_assert!(vidx1 != vidx2 && vidx1 != vidx3 && vidx2 != vidx3);
        self.polygon_array.push([vidx1, vidx2, vidx3]);
        for pass in 0..model.pass_count {
            if model.passes[pass].shader_array.is_some() {
                let s = model.get_shader(polygon_index, pass)?;
                self.shader_array[pass].as_mut().unwrap().push(s);
            }
            if model.passes[pass].material_array.is_some() {
                let first_vertex = model.polys[polygon_index][0] as usize;
                let m = model.peek_material(first_vertex, pass)?;
                self.material_array[pass].as_mut().unwrap().push(m);
            }
            for stage in 0..MAX_TEX_STAGES {
                if model.passes[pass].texture_array[stage].is_some() {
                    let t = model.peek_texture(polygon_index, pass, stage)?;
                    self.texture_array[pass][stage].as_mut().unwrap().push(t);
                }
            }
        }
        Ok(())
    }

    /// `GapFillerClass::Shrink_Buffers` (meshmdl.cpp:604): the buffers already hold exactly `PolygonCount` entries
    /// (they are `Vec`s), so only the recorded capacity changes.
    pub fn shrink_buffers(&mut self) {
        if self.polygon_array.len() == self.array_size {
            return;
        }
        self.array_size = self.polygon_array.len();
    }
}

/// `TriangleSide` (meshmdl.cpp:367): an undirected edge whose two end locations are ordered by the same integer
/// hash upstream uses (`i = 37*i + bits`, in `int`, wrapping), compared bitwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct TriangleSide {
    loc1: [u32; 3],
    loc2: [u32; 3],
}

fn bits(l: &[f32; 3]) -> [u32; 3] {
    [l[0].to_bits(), l[1].to_bits(), l[2].to_bits()]
}

fn side_hash(b: &[u32; 3]) -> i32 {
    let mut i = b[0] as i32;
    i = i.wrapping_mul(37).wrapping_add(b[1] as i32);
    i = i.wrapping_mul(37).wrapping_add(b[2] as i32);
    i
}

impl TriangleSide {
    fn new(l1: &[f32; 3], l2: &[f32; 3]) -> Self {
        let (b1, b2) = (bits(l1), bits(l2));
        if side_hash(&b1) < side_hash(&b2) {
            TriangleSide { loc1: b1, loc2: b2 }
        } else {
            TriangleSide { loc2: b1, loc1: b2 }
        }
    }
}

/// `SideIndexInfo` (meshmdl.cpp:413).
#[derive(Clone, Copy, Debug)]
struct SideIndexInfo {
    vidx1: u16,
    vidx2: u16,
    polygon_index: u32,
}

/// `MeshModelClass::Init_For_NPatch_Rendering` (meshmdl.cpp:653), after the caps tests (`Support_NPatches`,
/// `ALLOW_NPATCHES` flag, `GapFiller` already set), which the caller does. Finds pairs of triangles that share an
/// edge by LOCATION but not by vertex index (hard edges), and adds the two gap polygons for each. Hash tables are
/// `BTreeMap`s keyed on the locations' bit patterns (upstream hashes the bytes of the `Vector3`). Returns the gap
/// filler, or `None` when no gap was found (upstream leaves `GapFiller` NULL).
pub fn init_for_npatch_rendering(model: &MeshModel) -> W3dResult<Option<GapFillerClass>> {
    let locations = &model.vertices;
    let polygon_indices = &model.polys;

    let mut location_hash: BTreeMap<[u32; 3], u32> = BTreeMap::new();
    let mut duplicate_location_hash: BTreeMap<[u32; 3], u32> = BTreeMap::new();
    let mut side_hash_map: BTreeMap<TriangleSide, SideIndexInfo> = BTreeMap::new();
    let mut gap_filler: Option<GapFillerClass> = None;

    for (i, loc) in locations.iter().enumerate() {
        let key = bits(loc);
        if location_hash.contains_key(&key) {
            if !duplicate_location_hash.contains_key(&key) {
                duplicate_location_hash.insert(key, i as u32);
            }
        } else {
            location_hash.insert(key, i as u32);
        }
    }

    for i in 0..polygon_indices.len() {
        let poly = polygon_indices[i];
        let duplicates = [
            duplicate_location_hash.contains_key(&bits(&locations[poly[0] as usize])),
            duplicate_location_hash.contains_key(&bits(&locations[poly[1] as usize])),
            duplicate_location_hash.contains_key(&bits(&locations[poly[2] as usize])),
        ];
        // Each of the three sides (0,1), (1,2), (2,0) is handled by the same block upstream.
        for &(a, b, dup_a, dup_b) in &[(0usize, 1usize, 0usize, 1usize), (1, 2, 1, 2), (2, 0, 2, 0)] {
            if duplicates[dup_a] && duplicates[dup_b] {
                let tri = TriangleSide::new(&locations[poly[a] as usize], &locations[poly[b] as usize]);
                if let Some(side_index) = side_hash_map.get(&tri).copied() {
                    let idx1 = side_index.vidx1 as u32;
                    let idx2 = side_index.vidx2 as u32;
                    let idx3 = poly[a] as u32;
                    let idx4 = poly[b] as u32;
                    // `!(idx1^idx3) | !(idx1^idx4) | ...` is C++ logical NOT (1 when equal): `diff` is true when
                    // any of the four pairs of indices is equal (the two sides share a vertex).
                    let diff = diff_logical(idx1, idx2, idx3, idx4);
                    if !diff {
                        let g = gap_filler.get_or_insert_with(|| GapFillerClass::new(model));
                        g.add_polygon(model, i, idx4, idx2, idx1)?;
                        g.add_polygon(model, side_index.polygon_index as usize, idx3, idx2, idx4)?;
                    }
                } else {
                    side_hash_map.insert(
                        tri,
                        SideIndexInfo { vidx1: poly[a], vidx2: poly[b], polygon_index: i as u32 },
                    );
                }
            }
        }
    }

    if let Some(g) = &mut gap_filler {
        g.shrink_buffers();
    }
    Ok(gap_filler)
}

// `diff_logical`: upstream's `!(a^b)` is the C++ logical NOT of an `unsigned` (1 when a == b, else 0), so
// `!(idx1^idx3) | !(idx1^idx4) | !(idx2^idx3) | !(idx2^idx4)` is "any of the four index pairs is equal". Gap polygons
// are added only when `!diff`: when the two sides share NO vertex index (a genuinely hard edge).
fn diff_logical(idx1: u32, idx2: u32, idx3: u32, idx4: u32) -> bool {
    (idx1 == idx3) | (idx1 == idx4) | (idx2 == idx3) | (idx2 == idx4)
}
