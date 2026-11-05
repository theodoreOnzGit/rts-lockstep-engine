// SPDX-License-Identifier: GPL-3.0-only
//! W3D files written in memory: a small writer for meshes, hierarchies,
//! HLODs and raw animations, for tests and generated placeholder art.
//!
//! New code: upstream writes W3D files with its 3ds Max exporter (not in the
//! release) and `Save_W3D` functions this port does not translate. The
//! layouts are those of w3d_file.h ([`super::w3d_file`]); the files are read
//! back by the translated loaders, which is what the tests rely on. Nothing
//! here comes from the game's files.

use super::w3d_file::*;

/// A chunk: id, length (with the "contains chunks" bit for containers), body.
pub fn chunk(id: u32, body: &[u8]) -> Vec<u8> {
    let mut v = id.to_le_bytes().to_vec();
    v.extend_from_slice(&(body.len() as u32).to_le_bytes());
    v.extend_from_slice(body);
    v
}

/// A container chunk (the sub-chunk bit set).
pub fn chunk_sub(id: u32, body: &[u8]) -> Vec<u8> {
    let mut v = id.to_le_bytes().to_vec();
    v.extend_from_slice(&(body.len() as u32 | 0x8000_0000).to_le_bytes());
    v.extend_from_slice(body);
    v
}

/// A name in `N` bytes, zero-padded (truncated to leave a terminator).
pub fn name<const N: usize>(s: &[u8]) -> [u8; N] {
    let mut n = [0u8; N];
    let k = s.len().min(N - 1);
    n[..k].copy_from_slice(&s[..k]);
    n
}

fn f32s(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// A texture stage of a [`MeshSpec`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StageSpec {
    /// The texture file name (`None`: no texture on the stage).
    pub texture: Option<Vec<u8>>,
}

/// A one-pass mesh: geometry, one vertex material, one shader, at most one
/// texture on stage 0, optional bone links (a skin) and vertex colours.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshSpec {
    pub container: Vec<u8>,
    pub name: Vec<u8>,
    /// The header's `Attributes` (`W3D_MESH_FLAG_*`).
    pub attributes: u32,
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub tris: Vec<[u32; 3]>,
    /// The stage-0 texture coordinates as the file stores them (U, V).
    pub uv: Vec<[f32; 2]>,
    /// Per-vertex bone indices (`W3D_CHUNK_VERTEX_INFLUENCES`).
    pub bones: Option<Vec<u16>>,
    /// Per-vertex colours (`W3D_CHUNK_DCG`), as packed bytes R, G, B, A in a
    /// `u32` read little-endian (so `0x80808080` is mid grey, alpha 0x80).
    pub dcg: Option<Vec<u32>>,
    pub ambient: [u8; 3],
    pub diffuse: [u8; 3],
    pub emissive: [u8; 3],
    pub opacity: f32,
    /// The `W3dShaderStruct`'s 16 bytes.
    pub shader: [u8; 16],
    pub stage0: StageSpec,
    /// Whether the pass names its vertex material.
    pub vertex_material_on_pass: bool,
}

impl MeshSpec {
    /// A unit quad in the z = 0 plane facing +z (two counter-clockwise
    /// triangles), UVs at its corners, a material of diffuse (255, 128, 0)
    /// and ambient white, an alpha-blended textured shader, texture
    /// `quad.tga` on stage 0.
    pub fn quad(container: &[u8], name: &[u8]) -> MeshSpec {
        MeshSpec {
            container: container.to_vec(),
            name: name.to_vec(),
            attributes: 0,
            vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            tris: vec![[0, 1, 2], [0, 2, 3]],
            uv: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            bones: None,
            dcg: None,
            ambient: [255, 255, 255],
            diffuse: [255, 128, 0],
            emissive: [0, 0, 0],
            opacity: 1.0,
            // DepthCompare LEQUAL, DepthMask on, ColorMask, DestBlend
            // ONE_MINUS_SRC_ALPHA, Fog, PriGradient MODULATE, SecGradient,
            // SrcBlend SRC_ALPHA, Texturing on
            shader: [3, 1, 1, 5, 0, 1, 0, 2, 1, 0, 0, 0, 0, 0, 0, 0],
            stage0: StageSpec { texture: Some(b"quad.tga".to_vec()) },
            vertex_material_on_pass: true,
        }
    }

    /// An axis-aligned box from `min` to `max`, 24 vertices (4 per face, so
    /// each face has its own normal and UVs), 12 counter-clockwise triangles
    /// seen from outside.
    pub fn cuboid(container: &[u8], name: &[u8], min: [f32; 3], max: [f32; 3]) -> MeshSpec {
        let mut s = MeshSpec::quad(container, name);
        s.vertices.clear();
        s.normals.clear();
        s.tris.clear();
        s.uv.clear();
        // (normal axis, sign)
        for axis in 0..3 {
            for sign in [1.0f32, -1.0] {
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                let base = s.vertices.len() as u32;
                let w = if sign > 0.0 { max[axis] } else { min[axis] };
                // corners in (u, v) order, counter-clockwise seen from the normal side
                let corners: [(bool, bool); 4] = if sign > 0.0 {
                    [(false, false), (true, false), (true, true), (false, true)]
                } else {
                    [(false, false), (false, true), (true, true), (true, false)]
                };
                for (cu, cv) in corners {
                    let mut p = [0.0f32; 3];
                    p[axis] = w;
                    p[u] = if cu { max[u] } else { min[u] };
                    p[v] = if cv { max[v] } else { min[v] };
                    s.vertices.push(p);
                    let mut n = [0.0f32; 3];
                    n[axis] = sign;
                    s.normals.push(n);
                    s.uv.push([f32::from(u8::from(cu)), f32::from(u8::from(cv))]);
                }
                s.tris.push([base, base + 1, base + 2]);
                s.tris.push([base, base + 2, base + 3]);
            }
        }
        s
    }

    /// The `W3D_CHUNK_MESH` chunk.
    pub fn to_w3d(&self) -> Vec<u8> {
        let nv = self.vertices.len() as u32;
        let mut h = W3dMeshHeader3Struct::from_bytes(&[0; W3dMeshHeader3Struct::SIZE]);
        h.version = W3D_CURRENT_MESH_VERSION;
        h.attributes = self.attributes;
        h.mesh_name = name(&self.name);
        h.container_name = name(&self.container);
        h.num_tris = self.tris.len() as u32;
        h.num_vertices = nv;
        let mut body = chunk(W3D_CHUNK_MESH_HEADER3, &h.to_bytes());
        body.extend(chunk(W3D_CHUNK_VERTICES, &f32s(&self.vertices.concat())));
        body.extend(chunk(W3D_CHUNK_VERTEX_NORMALS, &f32s(&self.normals.concat())));
        let mut tris = Vec::new();
        for t in &self.tris {
            let mut s = W3dTriStruct::from_bytes(&[0; W3dTriStruct::SIZE]);
            s.vindex = *t;
            tris.extend(s.to_bytes());
        }
        body.extend(chunk(W3D_CHUNK_TRIANGLES, &tris));
        if let Some(b) = &self.bones {
            let mut inf = Vec::new();
            for &bone in b {
                let mut s = W3dVertInfStruct::from_bytes(&[0; W3dVertInfStruct::SIZE]);
                s.bone_idx = bone;
                inf.extend(s.to_bytes());
            }
            body.extend(chunk(W3D_CHUNK_VERTEX_INFLUENCES, &inf));
        }
        let ntex = u32::from(self.stage0.texture.is_some());
        let mut mi = W3dMaterialInfoStruct::from_bytes(&[0; W3dMaterialInfoStruct::SIZE]);
        mi.pass_count = 1;
        mi.vertex_material_count = 1;
        mi.shader_count = 1;
        mi.texture_count = ntex;
        body.extend(chunk(W3D_CHUNK_MATERIAL_INFO, &mi.to_bytes()));
        body.extend(chunk(W3D_CHUNK_SHADERS, &self.shader));
        let mut vm = W3dVertexMaterialStruct::from_bytes(&[0; W3dVertexMaterialStruct::SIZE]);
        let rgb = |c: [u8; 3]| W3dRGBStruct { r: c[0], g: c[1], b: c[2], pad: 0 };
        vm.ambient = rgb(self.ambient);
        vm.diffuse = rgb(self.diffuse);
        vm.emissive = rgb(self.emissive);
        vm.opacity = self.opacity;
        let mut vmc = chunk(W3D_CHUNK_VERTEX_MATERIAL_NAME, b"vmat\0");
        vmc.extend(chunk(W3D_CHUNK_VERTEX_MATERIAL_INFO, &vm.to_bytes()));
        body.extend(chunk_sub(W3D_CHUNK_VERTEX_MATERIALS, &chunk_sub(W3D_CHUNK_VERTEX_MATERIAL, &vmc)));
        if let Some(t) = &self.stage0.texture {
            let mut n = t.clone();
            n.push(0);
            body.extend(chunk_sub(W3D_CHUNK_TEXTURES, &chunk_sub(W3D_CHUNK_TEXTURE, &chunk(W3D_CHUNK_TEXTURE_NAME, &n))));
        }
        let mut pass = Vec::new();
        if self.vertex_material_on_pass {
            pass.extend(chunk(W3D_CHUNK_VERTEX_MATERIAL_IDS, &0u32.to_le_bytes()));
        }
        pass.extend(chunk(W3D_CHUNK_SHADER_IDS, &0u32.to_le_bytes()));
        if let Some(d) = &self.dcg {
            pass.extend(chunk(W3D_CHUNK_DCG, &d.iter().flat_map(|c| c.to_le_bytes()).collect::<Vec<u8>>()));
        }
        if self.stage0.texture.is_some() {
            let mut st = chunk(W3D_CHUNK_TEXTURE_IDS, &0u32.to_le_bytes());
            st.extend(chunk(W3D_CHUNK_STAGE_TEXCOORDS, &f32s(&self.uv.concat())));
            pass.extend(chunk_sub(W3D_CHUNK_TEXTURE_STAGE, &st));
        }
        body.extend(chunk_sub(W3D_CHUNK_MATERIAL_PASS, &pass));
        chunk_sub(W3D_CHUNK_MESH, &body)
    }
}

/// A pivot of [`htree_w3d`]: name, parent (-1 for none), translation and
/// rotation quaternion (x, y, z, w).
#[derive(Clone, Debug, PartialEq)]
pub struct PivotSpec {
    pub name: Vec<u8>,
    pub parent: i32,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
}

/// A `W3D_CHUNK_HIERARCHY` (version 4.1, so no root is added on load: give
/// `RootTransform` as pivot 0 yourself).
pub fn htree_w3d(tree_name: &[u8], pivots: &[PivotSpec]) -> Vec<u8> {
    let mut h = W3dHierarchyStruct::from_bytes(&[0; W3dHierarchyStruct::SIZE]);
    h.version = W3D_CURRENT_HTREE_VERSION;
    h.name = name(tree_name);
    h.num_pivots = pivots.len() as u32;
    let mut body = chunk(W3D_CHUNK_HIERARCHY_HEADER, &h.to_bytes());
    let mut ps = Vec::new();
    for p in pivots {
        let mut s = W3dPivotStruct::from_bytes(&[0; W3dPivotStruct::SIZE]);
        s.name = name(&p.name);
        s.parent_idx = p.parent as u32;
        s.translation = W3dVectorStruct { x: p.translation[0], y: p.translation[1], z: p.translation[2] };
        s.rotation.q = p.rotation;
        ps.extend(s.to_bytes());
    }
    body.extend(chunk(W3D_CHUNK_PIVOTS, &ps));
    chunk_sub(W3D_CHUNK_HIERARCHY, &body)
}

/// A `W3D_CHUNK_HLOD` with one LOD of `subs` (full sub-object name, bone).
pub fn hlod_w3d(hlod_name: &[u8], hierarchy: &[u8], subs: &[(&[u8], u32)]) -> Vec<u8> {
    let mut h = W3dHLodHeaderStruct::from_bytes(&[0; W3dHLodHeaderStruct::SIZE]);
    h.version = w3d_make_version(1, 0);
    h.lod_count = 1;
    h.name = name(hlod_name);
    h.hierarchy_name = name(hierarchy);
    let mut a = W3dHLodArrayHeaderStruct::from_bytes(&[0; W3dHLodArrayHeaderStruct::SIZE]);
    a.model_count = subs.len() as u32;
    a.max_screen_size = 0.0;
    let mut arr = chunk(W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER, &a.to_bytes());
    for (n, bone) in subs {
        let mut s = W3dHLodSubObjectStruct::from_bytes(&[0; W3dHLodSubObjectStruct::SIZE]);
        s.bone_index = *bone;
        s.name = name(n);
        arr.extend(chunk(W3D_CHUNK_HLOD_SUB_OBJECT, &s.to_bytes()));
    }
    let mut body = chunk(W3D_CHUNK_HLOD_HEADER, &h.to_bytes());
    body.extend(chunk_sub(W3D_CHUNK_HLOD_LOD_ARRAY, &arr));
    chunk_sub(W3D_CHUNK_HLOD, &body)
}

/// A channel of [`raw_anim_w3d`]: pivot, `ANIM_CHANNEL_*` type, vector
/// length, and the values of frames `0..` (`vector_len` floats each).
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelSpec {
    pub pivot: u16,
    pub kind: u32,
    pub vector_len: u16,
    pub data: Vec<f32>,
}

/// A raw `W3D_CHUNK_ANIMATION` `hierarchy.anim_name`.
pub fn raw_anim_w3d(hierarchy: &[u8], anim_name: &[u8], num_frames: u32, frame_rate: u32, channels: &[ChannelSpec]) -> Vec<u8> {
    let mut h = W3dAnimHeaderStruct::from_bytes(&[0; W3dAnimHeaderStruct::SIZE]);
    h.version = w3d_make_version(4, 1);
    h.name = name(anim_name);
    h.hierarchy_name = name(hierarchy);
    h.num_frames = num_frames;
    h.frame_rate = frame_rate;
    let mut body = chunk(W3D_CHUNK_ANIMATION_HEADER, &h.to_bytes());
    for c in channels {
        let frames = c.data.len() as u16 / c.vector_len.max(1);
        let mut b = Vec::new();
        b.extend(0u16.to_le_bytes()); // FirstFrame
        b.extend(frames.saturating_sub(1).to_le_bytes()); // LastFrame
        b.extend(c.vector_len.to_le_bytes());
        b.extend((c.kind as u16).to_le_bytes());
        b.extend(c.pivot.to_le_bytes());
        b.extend(0u16.to_le_bytes());
        b.extend(f32s(&c.data));
        body.extend(chunk(W3D_CHUNK_ANIMATION_CHANNEL, &b));
    }
    chunk_sub(W3D_CHUNK_ANIMATION, &body)
}
