// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshmdlio.cpp
//   (MeshModelClass::Load_W3D and the read_* functions it calls, the
//   MeshLoadContextClass members they use; since #67 read_v3_materials,
//   read_per_tri_materials and the legacy-material members Add_Legacy_Material,
//   Peek_Legacy_Shader, Peek_Legacy_Vertex_Material, Peek_Legacy_Texture)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshgeometry.cpp, .h
//   (Reset_Geometry, Set_Name, read_vertices, read_vertex_normals,
//   read_triangles, read_user_text, read_vertex_influences,
//   read_vertex_shade_indices, read_aabtree, the array accessors)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshmatdesc.cpp, .h
//   (the MeshMatDescClass state the loader sets and queries:
//   Reset, Install_UV_Array, the Set_/Has_/Get_ functions)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/vertmaterial.cpp
//   (VertexMaterialClass::Load_W3D, the reading; #67: the constructor's
//   D3DMATERIAL8, Init_From_Material3, Compute_CRC)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/shader.cpp
//   (#67: ShaderClass::Init_From_Material3)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/assetmgr.cpp
//   (#67: WW3DAssetManager::Get_Texture's refusal of an empty name)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/texture.cpp
//   (Load_Texture, the reading)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/aabtree.cpp
//   (AABTreeClass::Load_W3D, Read_Poly_Indices, Read_Nodes)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/w3d_util.cpp
//   (W3dUtilityClass::Convert_Shader, Convert_Color)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/shader.h
//   (ShaderClass's bits: Reset and the Set_/Get_ functions used)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/dx8wrapper.h
//   (DX8Wrapper::Convert_Color, both directions)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, by Greg Hjelstrom, Jani Penttinen and others)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! W3D meshes (`W3D_CHUNK_MESH`): `MeshModelClass::Load_W3D`, reading into
//! plain structs what the loader puts into the mesh model and its loading
//! context. No rendering: textures are names, vertex materials and shaders
//! are their file data (plus the shader bits `ShaderClass` packs them into).
//!
//! # As upstream reads it
//!
//! Inside the mesh chunk: a first chunk that must be `W3D_CHUNK_MESH_HEADER3`
//! ([`W3dMeshHeader3Struct`]), which sizes the geometry (`NumTris`,
//! `NumVertices`), names the mesh `Container.Mesh`, and sets flags; then
//! any chunks, each handled by its `read_*` function: vertices, normals,
//! triangles, user text, vertex influences (bones), shade indices, the
//! material info, shaders, vertex materials, textures, material passes
//! (vertex material ids, shader ids, DCG/DIG colours, texture stages with
//! texture ids and texture coordinates), the pre-lit material wrappers (only
//! the one matching the pre-lit mode is read), the AABTree, and the legacy
//! texture coordinates, vertex colours and per-triangle materials.
//!
//! Material data goes into the mesh's default material description
//! ([`Mesh::default_mat`]), or into the context's alternate one
//! ([`Mesh::alternate_mat`]) when the default already has that kind of data
//! for the pass (`MeshMatDescClass`, with 4 passes and 2 texture stages).
//! Texture-coordinate arrays are shared by CRC ([`crate::ww::realcrc`]).
//! Colours are packed as `DX8Wrapper::Convert_Color` packs them (ARGB, each
//! channel `trunc(value * 255)`; the product is exact at the x87 FPU's 53- or
//! 64-bit precision, and truncation keeps that result at 24 bits too).
//!
//! # Upstream quirks that are kept
//!
//! - **`read_chunks`' result is ignored**: a failing chunk only stops the
//!   reading, and the mesh still loads. `Load_W3D` fails only on the header.
//! - Several reads are not checked (vertex material, shader and texture ids,
//!   DCG/DIG colours, stage texture coordinates, the AABTree header, nodes
//!   and polygon indices, texture names and infos): a failed read leaves the
//!   previous value (with any bytes a short read copied) and the loop goes
//!   on. The value read before the first one is uninitialised (undefined).
//! - On a failed read, `read_material_pass` and `read_texture_stage`
//!   return without closing the chunk they opened, and `read_chunks` /
//!   `read_prelit_material` close theirs first; `Load_Texture` leaves a
//!   non-`W3D_CHUNK_TEXTURE` chunk open; the AABTree loader closes its header
//!   chunk even when it did not open one (closing the AABTree chunk itself).
//! - Triangle indices are stored as 16 bits (`TriIndex` is `Vector3i16`);
//!   surface types as the low byte of the attributes; plane distances
//!   negated. The planes go into a buffer shared by all meshes
//!   (`OPTIMIZE_PLANEEQ_RAM`), so upstream discards them; they are kept here.
//! - A vertex chunk with no geometry arrays (`NumTris` or `NumVertices` 0)
//!   is undefined (`Vertex->Get_Array()` of a null array); so are the
//!   normals and triangles.
//! - Only the first user text chunk and the first DCG colour array of a
//!   material description are kept (later DCG chunks only replace the alpha,
//!   and only in the pre-lit vertex mode); a second DIG goes to the
//!   alternate description and multiplies into its colour array.
//! - A single id (chunk length 4) sets the pass's single material, shader or
//!   texture; otherwise one per vertex (materials) or per polygon. A texture
//!   id of `0xFFFFFFFF` leaves the polygon without one.
//! - Pass 0 shaders that blend (`DstBlend` not zero) without alpha test set
//!   the `SORT` flag if the header's sort level is 0, from the shader *bits*
//!   (`ShaderClass` keeps 3 bits of `DstBlend` and 1 of `AlphaTest`, and its
//!   setters OR the whole value in: out-of-range values spill into other
//!   fields).
//! - Pre-3.0 skins add 1 to every bone index after loading.
//! - (#67) A texture chunk whose name is empty and that has no info chunk
//!   ends the textures chunk: `Get_Texture` returns null for an empty name
//!   and `read_textures` stops at a null texture. A legacy map with an empty
//!   name leaves its material without a texture (and still enables
//!   texturing).
//! - (#67) Legacy materials: the vertex-material search of
//!   `Add_Legacy_Material` (see `Mesh::add_legacy_material`); the SORT flag
//!   is set from the shader before an SI map makes it additive; the SI map is
//!   read only when the diffuse colour times its coefficients is black, and
//!   is otherwise skipped unread; the per-triangle chunk's single-kind
//!   defaults peek legacy material 0 whatever the ids say.
//! - The mesh's sort level is a `char`: the header's `int` truncated.
//! - Arrays are the size the header gives: whatever the file does not
//!   fill keeps the allocator's zeros (the game's allocator zero-fills).
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`W3dError::Undefined`](super::W3dError::Undefined): a pass index of 4 or more, or a stage of 2 or
//! more, used on the material arrays; an id at or past the number of
//! vertex materials, shaders or textures loaded; an uninitialised value read
//! (above); a name chunk over 256 bytes, or a name with no terminator in
//! what was read; a C string running past the end of the header; a negative
//! count or an allocation the 32-bit original cannot make; a ninth distinct
//! texture-coordinate array (`Get_UV_Array_Count` reads `UV[8]`); writes past
//! the temporary UV array; geometry chunks with no geometry arrays; (#67) a
//! legacy material id at or past the legacy materials read (`Peek_Legacy_*`
//! assert only), including legacy material 0 of a mesh that read none; a
//! legacy material or map name with no terminator in what was read; a
//! texture chunk with an info chunk and an empty name (`Get_Texture` returns
//! null and its filter is set).
//!
//! # What is left out
//!
//! - ~~The legacy (pre-W3D 3) material system: `W3D_CHUNK_MATERIALS3`
//!   (`read_v3_materials`, with its legacy material table de-duplicated by
//!   vertex-material CRC) and multi-material `W3D_CHUNK_PER_TRI_MATERIALS`.
//!   These give `W3dError::Unsupported`. (Single-material files skip the
//!   per-triangle chunk upstream, which is reproduced.)~~ **Corrected
//!   2026-11-14 (#67):** both are translated (`read_v3_materials`,
//!   `read_per_tri_materials`, `Add_Legacy_Material` as
//!   `Mesh::add_legacy_material`): a legacy material becomes a vertex
//!   material ([`VertexMaterial::legacy`], its CRC in
//!   [`Mesh::vertex_material_crcs`]), a shader and a texture, de-duplicated
//!   as upstream does, and indexed by [`Mesh::legacy_materials`]. The mesh
//!   loader has no `Unsupported` left.
//! - Everything after the chunks are read: generating an AABTree for
//!   collidable meshes without one (flagged:
//!   [`Mesh::needs_generated_cull_tree`]), `install_materials`
//!   (`Init_Alternate`, `Post_Load_Process`), and `post_process` (skins drop
//!   their AABTree, two-sided meshes disable culling, fog, static sort
//!   levels, overbright). **Corrected 2026-11-05 (#10, step 4):**
//!   `install_materials`' `Post_Load_Process` on the default description and
//!   `post_process` with the game's settings are translated in
//!   [`super::meshmdl::MeshModel::from_mesh`]; `Init_Alternate` and the
//!   AABTree are still left out.
//! - `VertexMaterialClass`'s conversion of its file data (`Parse_W3dVertex
//!   MaterialStruct`, `Parse_Mapping_Args`: the mapper argument strings are
//!   kept raw), and the texture objects themselves (`Get_Texture` loads image
//!   files; here a texture is its name and the settings `Load_Texture`
//!   derives). **Corrected 2026-11-05 (#10, step 4):**
//!   `Parse_W3dVertexMaterialStruct` and the mapper choice are translated in
//!   [`super::vertmaterial`] (the mapper arguments are still kept raw), and
//!   the image files a texture name leads to are found and decoded by
//!   [`super::render::load_texture_image`].

use super::w3d_file::*;
use super::{c_string, W3dResult, W3dUndefined, WIN32_MAX_ALLOCATION};
use crate::ww::chunkio::ChunkLoad;
use crate::logic::modules::plant;
use crate::ww::realcrc;

// ---------------------------------------------------------------------------
// Arrays sized by header counts
// ---------------------------------------------------------------------------

/// An array of `len` elements as upstream allocates it (zero-filled) and
/// fills it, stored as runs: stretches of individually written values, and
/// stretches of one repeated value (the allocator's zeros, or the value an
/// unchecked read loop kept writing once its reads had failed). Memory grows
/// with what the file holds, not with the header's counts.
#[derive(Clone, Debug, PartialEq)]
pub struct Arr<T> {
    len: usize,
    segs: Vec<Seg<T>>,
}

/// A stretch of an [`Arr`].
#[derive(Clone, Debug, PartialEq)]
pub enum Seg<T> {
    /// Values written one by one.
    Values(Vec<T>),
    /// `n` copies of one value.
    Run(usize, T),
}

impl<T> Seg<T> {
    fn len(&self) -> usize {
        match self {
            Seg::Values(v) => v.len(),
            Seg::Run(n, _) => *n,
        }
    }
}

impl<T: Copy + PartialEq> Arr<T> {
    pub(crate) fn new(len: usize, zero: T) -> Self {
        Arr { len, segs: if len > 0 { vec![Seg::Run(len, zero)] } else { Vec::new() } }
    }
    /// The number of elements.
    pub fn len(&self) -> usize {
        self.len
    }
    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// The stretches, in order.
    pub fn segments(&self) -> &[Seg<T>] {
        &self.segs
    }
    /// Element `i` (`i < len()`).
    pub fn get(&self, mut i: usize) -> T {
        assert!(i < self.len, "index {i} of {}", self.len);
        for s in &self.segs {
            if i < s.len() {
                return match s {
                    Seg::Values(v) => v[i],
                    Seg::Run(_, t) => *t,
                };
            }
            i -= s.len();
        }
        unreachable!()
    }
    /// Every element, in order (for small arrays).
    pub fn to_vec(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.len);
        for s in &self.segs {
            match s {
                Seg::Values(v) => out.extend_from_slice(v),
                Seg::Run(n, t) => out.extend(std::iter::repeat_n(*t, *n)),
            }
        }
        out
    }
    /// Splits so that a segment boundary falls at `at`; returns the index of
    /// the first segment at or after it.
    fn split(&mut self, at: usize) -> usize {
        let mut pos = 0;
        for k in 0..self.segs.len() {
            let n = self.segs[k].len();
            if at == pos {
                return k;
            }
            if at < pos + n {
                let off = at - pos;
                let tail = match &mut self.segs[k] {
                    Seg::Values(v) => Seg::Values(v.split_off(off)),
                    Seg::Run(m, t) => {
                        let t = *t;
                        *m = off;
                        Seg::Run(n - off, t)
                    }
                };
                self.segs.insert(k + 1, tail);
                return k + 1;
            }
            pos += n;
        }
        self.segs.len()
    }
    fn merge(&mut self) {
        let mut out: Vec<Seg<T>> = Vec::with_capacity(self.segs.len());
        for s in self.segs.drain(..) {
            if s.len() == 0 {
                continue;
            }
            match (out.last_mut(), s) {
                (Some(Seg::Values(a)), Seg::Values(b)) => a.extend(b),
                (Some(Seg::Run(n, t)), Seg::Run(m, u)) if *t == u => *n += m,
                (_, s) => out.push(s),
            }
        }
        self.segs = out;
    }
    pub(crate) fn set(&mut self, i: usize, v: T) {
        assert!(i < self.len);
        // fast path: the end of a values stretch, or inside one
        let mut pos = 0;
        for k in 0..self.segs.len() {
            let n = self.segs[k].len();
            if i < pos + n {
                if let Seg::Values(vals) = &mut self.segs[k] {
                    vals[i - pos] = v;
                    return;
                }
                if i == pos && k > 0 && matches!(self.segs[k - 1], Seg::Values(_)) {
                    if let Seg::Values(prev) = &mut self.segs[k - 1] {
                        prev.push(v);
                    }
                    if let Seg::Run(m, _) = &mut self.segs[k] {
                        *m -= 1;
                        if *m == 0 {
                            self.segs.remove(k);
                        }
                    }
                    return;
                }
                break;
            }
            pos += n;
        }
        self.replace(i, i + 1, Seg::Values(vec![v]));
    }
    fn replace(&mut self, a: usize, b: usize, seg: Seg<T>) {
        let ka = self.split(a);
        let kb = self.split(b);
        self.segs.splice(ka..kb, std::iter::once(seg));
        self.merge();
    }
    /// Elements `a..b` all become `v`.
    fn fill_range(&mut self, a: usize, b: usize, v: T) {
        let b = b.min(self.len);
        if a < b {
            self.replace(a, b, Seg::Run(b - a, v));
        }
    }
    /// Elements `i..` all become `v`.
    fn fill_from(&mut self, i: usize, v: T) {
        self.fill_range(i, self.len, v);
    }
    /// Elements `i..` become `f(element)`.
    fn map_from(&mut self, i: usize, f: impl Fn(T) -> T) {
        if i >= self.len {
            return;
        }
        let k = self.split(i);
        for s in &mut self.segs[k..] {
            match s {
                Seg::Values(v) => v.iter_mut().for_each(|x| *x = f(*x)),
                Seg::Run(_, t) => *t = f(*t),
            }
        }
        self.merge();
    }
    /// A copy of the first `n` elements.
    fn head(&self, n: usize) -> Arr<T> {
        let mut c = self.clone();
        let k = c.split(n.min(self.len));
        c.segs.truncate(k);
        c.len = n.min(self.len);
        c
    }
}

fn alloc(count: i32, elem: u64) -> W3dResult<usize> {
    if count < 0 || count as u64 * elem > WIN32_MAX_ALLOCATION {
        return Err(W3dUndefined::Allocation.into());
    }
    Ok(count as usize)
}

/// A local variable read by unchecked reads: its bytes, and how many of them
/// have been written (reads always start at its first byte).
struct Stale<const N: usize> {
    b: [u8; N],
    init: usize,
}

impl<const N: usize> Stale<N> {
    fn new() -> Self {
        Stale { b: [0; N], init: 0 }
    }
    /// `cload.Read(&v, N)`: whether it read all `N` bytes (a short read
    /// still copies what the file had).
    fn read(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool> {
        let p0 = cload.file().position();
        let r = cload.read(&mut self.b, N as u32)?;
        let copied = (i64::from(cload.file().position()) - i64::from(p0)).max(0) as usize;
        self.init = self.init.max(copied.min(N));
        Ok(r == N as u32)
    }
    fn get(&self) -> W3dResult<[u8; N]> {
        if self.init < N {
            return Err(W3dUndefined::UninitialisedRead.into());
        }
        Ok(self.b)
    }
}

fn u32le(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

// ---------------------------------------------------------------------------
// ShaderClass bits, colours
// ---------------------------------------------------------------------------

/// `ShaderClass`'s field masks and shifts (shader.h).
pub mod shader_bits {
    pub const MASK_DEPTHCOMPARE: u32 = 7;
    pub const MASK_DEPTHMASK: u32 = 1 << 3;
    pub const MASK_COLORMASK: u32 = 1 << 4;
    pub const MASK_DSTBLEND: u32 = 7 << 5;
    pub const MASK_FOG: u32 = 3 << 8;
    pub const MASK_PRIGRADIENT: u32 = 7 << 10;
    pub const MASK_SECGRADIENT: u32 = 1 << 13;
    pub const MASK_SRCBLEND: u32 = 3 << 14;
    pub const MASK_TEXTURING: u32 = 1 << 16;
    pub const MASK_NPATCHENABLE: u32 = 1 << 17;
    pub const MASK_ALPHATEST: u32 = 1 << 18;
    pub const MASK_CULLMODE: u32 = 1 << 19;
    pub const MASK_POSTDETAILCOLORFUNC: u32 = 15 << 20;
    pub const MASK_POSTDETAILALPHAFUNC: u32 = 7 << 24;
    pub const SHIFT_DEPTHCOMPARE: u32 = 0;
    pub const SHIFT_DEPTHMASK: u32 = 3;
    pub const SHIFT_COLORMASK: u32 = 4;
    pub const SHIFT_DSTBLEND: u32 = 5;
    pub const SHIFT_FOG: u32 = 8;
    pub const SHIFT_PRIGRADIENT: u32 = 10;
    pub const SHIFT_SECGRADIENT: u32 = 13;
    pub const SHIFT_SRCBLEND: u32 = 14;
    pub const SHIFT_TEXTURING: u32 = 16;
    pub const SHIFT_NPATCHENABLE: u32 = 17;
    pub const SHIFT_ALPHATEST: u32 = 18;
    pub const SHIFT_CULLMODE: u32 = 19;
    pub const SHIFT_POSTDETAILCOLORFUNC: u32 = 20;
    pub const SHIFT_POSTDETAILALPHAFUNC: u32 = 24;
}
use shader_bits::*;

/// A `ShaderClass::Set_*`: clear the field, OR in `x << shift` (unmasked).
fn shader_set(bits: &mut u32, mask: u32, shift: u32, x: u32) {
    *bits &= !mask;
    *bits |= x << shift;
}

/// `ShaderClass()` (`Reset`).
pub fn shader_reset() -> u32 {
    let mut b = 0;
    shader_set(&mut b, MASK_DEPTHCOMPARE, SHIFT_DEPTHCOMPARE, 3); // PASS_LEQUAL
    shader_set(&mut b, MASK_DEPTHMASK, SHIFT_DEPTHMASK, 1); // DEPTH_WRITE_ENABLE
    shader_set(&mut b, MASK_COLORMASK, SHIFT_COLORMASK, 1); // COLOR_WRITE_ENABLE
    shader_set(&mut b, MASK_DSTBLEND, SHIFT_DSTBLEND, 0);
    shader_set(&mut b, MASK_FOG, SHIFT_FOG, 0);
    shader_set(&mut b, MASK_PRIGRADIENT, SHIFT_PRIGRADIENT, 1); // GRADIENT_MODULATE
    shader_set(&mut b, MASK_SECGRADIENT, SHIFT_SECGRADIENT, 0);
    shader_set(&mut b, MASK_SRCBLEND, SHIFT_SRCBLEND, 1); // SRCBLEND_ONE
    shader_set(&mut b, MASK_TEXTURING, SHIFT_TEXTURING, 0);
    shader_set(&mut b, MASK_ALPHATEST, SHIFT_ALPHATEST, 0);
    shader_set(&mut b, MASK_CULLMODE, SHIFT_CULLMODE, 1); // CULL_MODE_ENABLE
    shader_set(&mut b, MASK_POSTDETAILCOLORFUNC, SHIFT_POSTDETAILCOLORFUNC, 0);
    shader_set(&mut b, MASK_POSTDETAILALPHAFUNC, SHIFT_POSTDETAILALPHAFUNC, 0);
    shader_set(&mut b, MASK_NPATCHENABLE, SHIFT_NPATCHENABLE, 0);
    b
}

/// `W3dUtilityClass::Convert_Shader(shader, &set)` on a new `ShaderClass`:
/// its bits.
pub fn convert_shader(s: &W3dShaderStruct) -> u32 {
    let mut b = shader_reset();
    shader_set(&mut b, MASK_DEPTHCOMPARE, SHIFT_DEPTHCOMPARE, u32::from(s.depth_compare));
    shader_set(&mut b, MASK_DEPTHMASK, SHIFT_DEPTHMASK, u32::from(s.depth_mask));
    shader_set(&mut b, MASK_COLORMASK, SHIFT_COLORMASK, 1);
    shader_set(&mut b, MASK_DSTBLEND, SHIFT_DSTBLEND, u32::from(s.dest_blend));
    shader_set(&mut b, MASK_FOG, SHIFT_FOG, 0);
    shader_set(&mut b, MASK_PRIGRADIENT, SHIFT_PRIGRADIENT, u32::from(s.pri_gradient));
    shader_set(&mut b, MASK_SECGRADIENT, SHIFT_SECGRADIENT, u32::from(s.sec_gradient));
    shader_set(&mut b, MASK_SRCBLEND, SHIFT_SRCBLEND, u32::from(s.src_blend));
    shader_set(&mut b, MASK_TEXTURING, SHIFT_TEXTURING, u32::from(s.texturing));
    shader_set(&mut b, MASK_ALPHATEST, SHIFT_ALPHATEST, u32::from(s.alpha_test));
    shader_set(&mut b, MASK_POSTDETAILCOLORFUNC, SHIFT_POSTDETAILCOLORFUNC, u32::from(s.detail_color_func));
    shader_set(&mut b, MASK_POSTDETAILALPHAFUNC, SHIFT_POSTDETAILALPHAFUNC, u32::from(s.detail_alpha_func));
    b
}

/// `fistp` with the rounding control at chop: the integer part, or the
/// integer indefinite `0x80000000` outside `int` (and for NaN).
fn fistp_chop(x: f64) -> u32 {
    let t = x.trunc();
    if t.is_nan() || !(-2147483648.0..2147483648.0).contains(&t) {
        return 0x8000_0000;
    }
    t as i32 as u32
}

/// `DX8Wrapper::Convert_Color(const Vector4 &)`: `(a<<24)|(r<<16)|(g<<8)|b`,
/// each `fistp(chop)` of the channel times 255 (exact, see the module
/// docs); the shifts are 32-bit and nothing is masked.
pub fn dx8_convert_color(c: [f32; 4]) -> u32 {
    let t = |v: f32| fistp_chop(f64::from(v) * 255.0);
    let (r, g, b, a) = (t(c[0]), t(c[1]), t(c[2]), t(c[3]));
    a.wrapping_shl(24) | b | r.wrapping_shl(16) | g.wrapping_shl(8)
}

/// `DX8Wrapper::Convert_Color(unsigned)`: the four channels over 255.
pub fn dx8_unpack_color(c: u32) -> [f32; 4] {
    [
        ((c & 0x00ff_0000) >> 16) as f32 / 255.0,
        ((c & 0x0000_ff00) >> 8) as f32 / 255.0,
        (c & 0x0000_00ff) as f32 / 255.0,
        ((c & 0xff00_0000) >> 24) as f32 / 255.0,
    ]
}

// ---------------------------------------------------------------------------
// The material description
// ---------------------------------------------------------------------------

/// `MeshMatDescClass::MAX_PASSES`.
pub const MAX_PASSES: usize = 4;
/// `MeshMatDescClass::MAX_TEX_STAGES`.
pub const MAX_TEX_STAGES: usize = 2;
/// `MeshMatDescClass::MAX_UV_ARRAYS`.
pub const MAX_UV_ARRAYS: usize = MAX_PASSES * MAX_TEX_STAGES;
/// `VertexMaterialClass::MATERIAL`.
pub const COLOR_SOURCE_MATERIAL: u32 = 0;
/// `VertexMaterialClass::COLOR1`.
pub const COLOR_SOURCE_COLOR1: u32 = 1;

/// One pass of a [`MatDesc`]. Vertex materials and textures are indices
/// into [`Mesh::vertex_materials`] and [`Mesh::textures`]; shaders are
/// `ShaderClass` bits.
#[derive(Clone, Debug, PartialEq)]
pub struct MatPass {
    /// `Material[pass]`.
    pub material: Option<u32>,
    /// `MaterialArray[pass]` (per vertex).
    pub material_array: Option<Arr<Option<u32>>>,
    /// `Shader[pass]` (0, `NullShader`, means none).
    pub shader: u32,
    /// `ShaderArray[pass]` (per polygon, zero-filled).
    pub shader_array: Option<Arr<u32>>,
    /// `Texture[pass][stage]`.
    pub texture: [Option<u32>; MAX_TEX_STAGES],
    /// `TextureArray[pass][stage]` (per polygon).
    pub texture_array: [Option<Arr<Option<u32>>>; MAX_TEX_STAGES],
    /// `UVSource[pass][stage]`: an index into [`MatDesc::uv`], or -1.
    pub uv_source: [i32; MAX_TEX_STAGES],
    /// `DCGSource[pass]` (`COLOR_SOURCE_*`).
    pub dcg_source: u32,
    /// `DIGSource[pass]`.
    pub dig_source: u32,
}

impl Default for MatPass {
    fn default() -> Self {
        MatPass {
            material: None,
            material_array: None,
            shader: 0,
            shader_array: None,
            texture: [None; MAX_TEX_STAGES],
            texture_array: [None, None],
            uv_source: [-1; MAX_TEX_STAGES],
            dcg_source: COLOR_SOURCE_MATERIAL,
            dig_source: COLOR_SOURCE_MATERIAL,
        }
    }
}

/// A texture-coordinate array (`UVBufferClass`): `(U, 1 - V)` pairs and
/// their `CRC_Memory`.
#[derive(Clone, Debug, PartialEq)]
pub struct UvArray {
    pub uvs: Arr<[f32; 2]>,
    pub crc: u32,
}

/// `class MeshMatDescClass`: the state the loader sets.
#[derive(Clone, Debug, PartialEq)]
pub struct MatDesc {
    pub pass_count: i32,
    pub vertex_count: i32,
    pub poly_count: i32,
    pub passes: [MatPass; MAX_PASSES],
    /// `UV[]`: the installed arrays (at most 8, filled in order).
    pub uv: Vec<UvArray>,
    /// `ColorArray[0..2]` (packed ARGB per vertex).
    pub color_array: [Option<Arr<u32>>; 2],
}

impl Default for MatDesc {
    /// `MeshMatDescClass()`.
    fn default() -> Self {
        MatDesc {
            pass_count: 1,
            vertex_count: 0,
            poly_count: 0,
            passes: Default::default(),
            uv: Vec::new(),
            color_array: [None, None],
        }
    }
}

fn pass_index(pass: i32) -> W3dResult<usize> {
    if (0..MAX_PASSES as i32).contains(&pass) {
        Ok(pass as usize)
    } else {
        Err(W3dUndefined::IndexOutOfRange.into())
    }
}

fn stage_index(stage: i32) -> W3dResult<usize> {
    if (0..MAX_TEX_STAGES as i32).contains(&stage) {
        Ok(stage as usize)
    } else {
        Err(W3dUndefined::IndexOutOfRange.into())
    }
}

impl MatDesc {
    fn has_material_data(&self, pass: i32) -> W3dResult<bool> {
        let p = &self.passes[pass_index(pass)?];
        Ok(p.material.is_some() || p.material_array.is_some())
    }
    fn has_shader_data(&self, pass: i32) -> W3dResult<bool> {
        let p = &self.passes[pass_index(pass)?];
        Ok(p.shader != 0 || p.shader_array.is_some())
    }
    fn has_texture_data(&self, pass: i32, stage: i32) -> W3dResult<bool> {
        let p = &self.passes[pass_index(pass)?];
        let s = stage_index(stage)?;
        Ok(p.texture[s].is_some() || p.texture_array[s].is_some())
    }
    fn has_uv(&self, pass: i32, stage: i32) -> W3dResult<bool> {
        Ok(self.passes[pass_index(pass)?].uv_source[stage_index(stage)?] != -1)
    }
    /// `Get_Color_Array(index, true)`.
    fn color_array(&mut self, index: usize) -> W3dResult<&mut Arr<u32>> {
        if self.color_array[index].is_none() {
            let n = alloc(self.vertex_count, 4)?;
            self.color_array[index] = Some(Arr::new(n, 0));
        }
        Ok(self.color_array[index].as_mut().unwrap())
    }
    /// `Set_Material(vidx, vmat, pass)` (`None`: a null material).
    fn set_material(&mut self, vidx: usize, vmat: Option<u32>, pass: i32) -> W3dResult<()> {
        let n = self.vertex_count;
        let p = &mut self.passes[pass_index(pass)?];
        if p.material_array.is_none() {
            p.material_array = Some(Arr::new(alloc(n, 4)?, None));
        }
        let a = p.material_array.as_mut().unwrap();
        if vidx >= a.len() {
            return Err(W3dUndefined::IndexOutOfRange.into());
        }
        a.set(vidx, vmat);
        Ok(())
    }
    fn shader_array(&mut self, pass: i32) -> W3dResult<&mut Arr<u32>> {
        let n = self.poly_count;
        let p = &mut self.passes[pass_index(pass)?];
        if p.shader_array.is_none() {
            p.shader_array = Some(Arr::new(alloc(n, 4)?, 0));
        }
        Ok(p.shader_array.as_mut().unwrap())
    }
    fn texture_array(&mut self, pass: i32, stage: i32) -> W3dResult<&mut Arr<Option<u32>>> {
        let n = self.poly_count;
        let s = stage_index(stage)?;
        let p = &mut self.passes[pass_index(pass)?];
        if p.texture_array[s].is_none() {
            p.texture_array[s] = Some(Arr::new(alloc(n, 4)?, None));
        }
        Ok(p.texture_array[s].as_mut().unwrap())
    }

    /// `Install_UV_Array(pass, stage, uvs, count)`: shares an installed
    /// array with the same CRC, else installs a copy in the first free slot
    /// (if any).
    fn install_uv_array(&mut self, pass: i32, stage: i32, uvs: &Arr<[f32; 2]>, count: i32) -> W3dResult<()> {
        let n = count.max(0) as usize;
        let crc = uv_crc(uvs, n);
        if self.uv.len() >= MAX_UV_ARRAYS {
            // Get_UV_Array_Count reads UV[MAX_UV_ARRAYS]
            return Err(W3dUndefined::IndexOutOfRange.into());
        }
        let (p, s) = (pass_index(pass)?, stage_index(stage)?);
        if let Some(i) = self.uv.iter().position(|u| u.crc == crc) {
            self.passes[p].uv_source[s] = i as i32;
            return Ok(());
        }
        alloc(count, 8)?;
        self.uv.push(UvArray { uvs: uvs.head(n), crc });
        self.passes[p].uv_source[s] = self.uv.len() as i32 - 1;
        Ok(())
    }
}

/// `CRC_Memory` of the first `n` elements of `uvs` as bytes.
fn uv_crc(uvs: &Arr<[f32; 2]>, n: usize) -> u32 {
    let bytes = |v: [f32; 2]| {
        let mut b = [0u8; 8];
        b[..4].copy_from_slice(&v[0].to_le_bytes());
        b[4..].copy_from_slice(&v[1].to_le_bytes());
        b
    };
    let mut state = 0xFFFF_FFFFu32;
    let mut left = n;
    for s in &uvs.segs {
        if left == 0 {
            break;
        }
        match s {
            Seg::Values(v) => {
                for x in v.iter().take(left) {
                    state = realcrc::crc_feed(state, &bytes(*x));
                }
                left -= v.len().min(left);
            }
            Seg::Run(m, t) => {
                let k = (*m).min(left);
                state = crc_feed_repeated(state, &bytes(*t), k);
                left -= k;
            }
        }
    }
    state ^ 0xFFFF_FFFF
}

/// [`realcrc::crc_feed`] of `block` repeated `times` times, in O(log times):
/// one block's update is affine over GF(2) (`s -> M s ^ c`), so the
/// repetition is that map raised to a power by squaring. The result is the
/// same as feeding the bytes one at a time (unit-tested below); runs of
/// millions of identical texture coordinates would otherwise take seconds.
fn crc_feed_repeated(state: u32, block: &[u8], times: usize) -> u32 {
    #[derive(Clone, Copy)]
    struct Affine {
        cols: [u32; 32],
        c: u32,
    }
    fn mat(cols: &[u32; 32], v: u32) -> u32 {
        (0..32).filter(|i| v >> i & 1 != 0).fold(0, |a, i| a ^ cols[i])
    }
    // f after g
    fn compose(f: &Affine, g: &Affine) -> Affine {
        let mut cols = [0u32; 32];
        for (i, col) in cols.iter_mut().enumerate() {
            *col = mat(&f.cols, g.cols[i]);
        }
        Affine { cols, c: mat(&f.cols, g.c) ^ f.c }
    }
    let c = realcrc::crc_feed(0, block);
    let mut base = Affine { cols: [0; 32], c };
    for (i, col) in base.cols.iter_mut().enumerate() {
        *col = realcrc::crc_feed(1 << i, block) ^ c;
    }
    let mut state = state;
    let mut n = times;
    while n != 0 {
        if n & 1 != 0 {
            state = mat(&base.cols, state) ^ base.c;
        }
        n >>= 1;
        if n != 0 {
            base = compose(&base, &base);
        }
    }
    state
}

// ---------------------------------------------------------------------------
// Vertex materials, textures, the AABTree
// ---------------------------------------------------------------------------

/// A vertex material as `VertexMaterialClass::Load_W3D` reads it, or (#67)
/// as `read_v3_materials` makes it from a legacy material.
#[derive(Clone, Debug, PartialEq)]
pub struct VertexMaterial {
    /// The name (`Set_Name`), if a name chunk was read.
    pub name: Option<Vec<u8>>,
    /// `W3D_CHUNK_VERTEX_MATERIAL_INFO` (all zeros for a legacy material).
    pub info: W3dVertexMaterialStruct,
    /// The mapper argument strings (`W3D_CHUNK_VERTEX_MAPPER_ARGS0`, `ARGS1`),
    /// as read (the last chunk of each kind).
    pub mapper_args: [Option<Vec<u8>>; 2],
    /// A legacy material's `D3DMATERIAL8`: `VertexMaterialClass()`, then
    /// `Init_From_Material3` and `read_v3_materials`' move of an untextured
    /// diffuse to ambient (#67). `None` for a material of
    /// `W3D_CHUNK_VERTEX_MATERIALS`, whose `info` is converted later
    /// ([`super::vertmaterial::VertexMaterialState::from_w3d`]).
    pub legacy: Option<D3dMaterial8>,
}

/// `D3DMATERIAL8` (d3d8types.h), the colours `VertexMaterialClass` keeps
/// behind `MaterialDyn`: `[r, g, b, a]` each.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D3dMaterial8 {
    pub diffuse: [f32; 4],
    pub ambient: [f32; 4],
    pub specular: [f32; 4],
    pub emissive: [f32; 4],
    pub power: f32,
}

impl D3dMaterial8 {
    /// `VertexMaterialClass::VertexMaterialClass`: `memset` to zero, then
    /// `Set_Ambient(1,1,1)`, `Set_Diffuse(1,1,1)`, `Set_Opacity(1)`.
    pub const NEW: D3dMaterial8 =
        D3dMaterial8 { diffuse: [1.0; 4], ambient: [1.0, 1.0, 1.0, 0.0], specular: [0.0; 4], emissive: [0.0; 4], power: 0.0 };

    /// The 68 bytes, in `D3DMATERIAL8`'s order.
    pub fn to_bytes(&self) -> [u8; 68] {
        let mut b = [0u8; 68];
        for (i, v) in self.diffuse.iter().chain(&self.ambient).chain(&self.specular).chain(&self.emissive).chain([&self.power]).enumerate() {
            b[4 * i..4 * i + 4].copy_from_slice(&v.to_le_bytes());
        }
        b
    }

    /// `VertexMaterialClass::Init_From_Material3(mat3)` on a new material:
    /// diffuse and specular are the colour times its coefficients
    /// (`Convert_Color` of each, a `float` product per channel), emissive
    /// and ambient the coefficients, then shininess and opacity (the diffuse
    /// alpha).
    pub fn init_from_material3(mat3: &W3dMaterial3Struct) -> Self {
        let cc = super::vertmaterial::convert_color;
        let mul = |a: [f32; 3], b: [f32; 3]| [a[0] * b[0], a[1] * b[1], a[2] * b[2]];
        let mut m = D3dMaterial8::NEW;
        let d = if plant(10085) { cc(&mat3.diffuse_color) } else { mul(cc(&mat3.diffuse_color), cc(&mat3.diffuse_coefficients)) };
        m.diffuse[..3].copy_from_slice(&d);
        let sp = mul(cc(&mat3.specular_color), cc(&mat3.specular_coefficients));
        m.specular[..3].copy_from_slice(&sp);
        m.emissive[..3].copy_from_slice(&cc(&mat3.emissive_coefficients));
        m.ambient[..3].copy_from_slice(&cc(&mat3.ambient_coefficients));
        m.power = mat3.shininess;
        if !plant(10086) {
            m.diffuse[3] = mat3.opacity;
        }
        m
    }
}

/// `VertexMaterialClass::Compute_CRC` of a material whose other members
/// are the constructor's (`Flags` 0, the three colour sources
/// `D3DMCS_MATERIAL`, `UVSource[i] = i`, `UseLighting` false, `UniqueID` 0,
/// no mapper), as every legacy material is when `Add_Legacy_Material` asks:
/// `CRC_Memory` over the `D3DMATERIAL8`, then each member by its own size,
/// chained from 0.
pub fn vertex_material_crc(m: &D3dMaterial8) -> u32 {
    let mut crc = realcrc::crc_memory(&m.to_bytes(), 0);
    for v in [0u32, 0, 0, 0] {
        // Flags, DiffuseColorSource, AmbientColorSource, EmissiveColorSource
        crc = realcrc::crc_memory(&v.to_le_bytes(), crc);
    }
    let mut uv = [0u8; 32];
    for i in 0..8 {
        uv[4 * i..4 * i + 4].copy_from_slice(&(i as u32).to_le_bytes());
    }
    crc = realcrc::crc_memory(&uv, crc);
    crc = realcrc::crc_memory(&[0u8], crc); // UseLighting
    realcrc::crc_memory(&0u32.to_le_bytes(), crc) // UniqueID
}

/// A `char name[N]` on the stack, read by name chunks.
struct NameBuf<const N: usize> {
    b: [u8; N],
    init: usize,
}

impl<const N: usize> NameBuf<N> {
    fn new() -> Self {
        NameBuf { b: [0; N], init: 0 }
    }
    /// `cload.Read(&name, cload.Cur_Chunk_Length())` (result not used).
    fn read(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<()> {
        let len = cload.cur_chunk_length()?;
        let p0 = cload.file().position();
        cload.read(&mut self.b, len)?;
        let copied = (i64::from(cload.file().position()) - i64::from(p0)).max(0) as usize;
        self.init = self.init.max(copied);
        Ok(())
    }
    /// The C string, which must end inside what was read.
    fn string(&self) -> W3dResult<Vec<u8>> {
        match self.b[..self.init].iter().position(|&c| c == 0) {
            Some(n) => Ok(self.b[..n].to_vec()),
            None => Err(W3dUndefined::UninitialisedRead.into()),
        }
    }
}

/// `VertexMaterialClass::Load_W3D(cload)`. `Ok(None)` is
/// `WW3D_ERROR_LOAD_FAILED`.
fn load_vertex_material(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<VertexMaterial>> {
    let mut name = NameBuf::<256>::new();
    let mut hasname = false;
    let mut vmat: Option<W3dVertexMaterialStruct> = None;
    let mut args: [Option<Vec<u8>>; 2] = [None, None];
    while cload.open_chunk()? {
        match cload.cur_chunk_id()? {
            W3D_CHUNK_VERTEX_MATERIAL_NAME => {
                name.read(cload)?;
                hasname = true;
            }
            W3D_CHUNK_VERTEX_MATERIAL_INFO => {
                let mut s = Stale::<{ W3dVertexMaterialStruct::SIZE }>::new();
                if let Some(v) = vmat {
                    s.b.copy_from_slice(&v.to_bytes());
                    s.init = W3dVertexMaterialStruct::SIZE;
                }
                if !s.read(cload)? {
                    return Ok(None);
                }
                vmat = Some(W3dVertexMaterialStruct::from_bytes(&s.b));
            }
            id @ (W3D_CHUNK_VERTEX_MAPPER_ARGS0 | W3D_CHUNK_VERTEX_MAPPER_ARGS1) => {
                let len = cload.cur_chunk_length()?;
                let a = match super::read_array(cload, len, len as usize)? {
                    Some(a) => a,
                    None => return Ok(None),
                };
                args[(id - W3D_CHUNK_VERTEX_MAPPER_ARGS0) as usize] = Some(a);
            }
            _ => {}
        }
        cload.close_chunk()?;
    }
    let name = if hasname { Some(name.string()?) } else { None };
    // Parse_W3dVertexMaterialStruct(vmat): uninitialised without an info chunk
    let info = vmat.ok_or(W3dUndefined::UninitialisedRead)?;
    Ok(Some(VertexMaterial { name, info, mapper_args: args, legacy: None }))
}

/// A texture as `Load_Texture` asks the asset manager for it.
#[derive(Clone, Debug, PartialEq)]
pub struct Texture {
    /// The file name.
    pub name: Vec<u8>,
    /// The texture info's `Attributes`, if there was an info chunk (the rest
    /// of the info is not used).
    pub attributes: Option<u16>,
    /// `MipCountType` passed to `Get_Texture` (0, `MIP_LEVELS_ALL`, without
    /// an info chunk).
    pub mip_count: u32,
    /// `W3DTEXTURE_NO_LOD`: mip-mapping filter set to none.
    pub no_lod: bool,
    /// U and V address modes clamped (only set with an info chunk).
    pub clamp: Option<(bool, bool)>,
}

/// `WW3DAssetManager::Get_Texture(filename)` with the default arguments, as
/// the legacy materials call it: `None` (null) for an empty name, which
/// upstream's first test refuses; otherwise the texture by its name (the
/// asset manager's lower-casing and sharing are not modelled: a name stands
/// for the texture, see [`super::render`]).
fn get_texture(name: Vec<u8>) -> Option<Texture> {
    if name.is_empty() {
        return None;
    }
    Some(Texture { name, attributes: None, mip_count: 0, no_lod: false, clamp: None })
}

/// `Load_Texture(cload)`: `Ok(None)` is a null texture (the end of the
/// textures chunk, or a chunk that is not a texture, left open; **corrected
/// 2026-11-14 (#67)**: or an empty name without an info chunk, for which
/// `Get_Texture` returns null; with an info chunk the null texture's filter
/// is then set, which is undefined).
fn load_texture(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<Texture>> {
    if !cload.open_chunk()? || cload.cur_chunk_id()? != W3D_CHUNK_TEXTURE {
        return Ok(None);
    }
    let mut name = NameBuf::<256>::new();
    let mut texinfo = Stale::<{ W3dTextureInfoStruct::SIZE }>::new();
    let mut hastexinfo = false;
    while cload.open_chunk()? {
        match cload.cur_chunk_id()? {
            W3D_CHUNK_TEXTURE_NAME => name.read(cload)?,
            W3D_CHUNK_TEXTURE_INFO => {
                texinfo.read(cload)?;
                hastexinfo = true;
            }
            _ => {}
        }
        cload.close_chunk()?;
    }
    cload.close_chunk()?;
    let (attributes, mip_count, no_lod, clamp) = if hastexinfo {
        // only Attributes (the first 2 bytes) is used
        if texinfo.init < 2 {
            return Err(W3dUndefined::UninitialisedRead.into());
        }
        let a = u16::from_le_bytes([texinfo.b[0], texinfo.b[1]]);
        let no_lod = a & W3DTEXTURE_NO_LOD == W3DTEXTURE_NO_LOD;
        let mip = if no_lod {
            1 // MIP_LEVELS_1
        } else {
            match a & W3DTEXTURE_MIP_LEVELS_MASK {
                0x0000 => 0, // MIP_LEVELS_ALL
                0x0040 => 2, // MIP_LEVELS_2
                0x0080 => 3,
                _ => 4, // 0x00c0: MIP_LEVELS_4
            }
        };
        (Some(a), mip, no_lod, Some((a & W3DTEXTURE_CLAMP_U != 0, a & W3DTEXTURE_CLAMP_V != 0)))
    } else {
        (None, 0, false, None)
    };
    let name = name.string()?;
    if name.is_empty() {
        // Get_Texture returned null
        if hastexinfo {
            return Err(W3dUndefined::NullPointer.into());
        }
        return Ok(None);
    }
    Ok(Some(Texture { name, attributes, mip_count, no_lod, clamp }))
}

/// `AABTreeClass::CullNodeStruct`, as read from a [`W3dMeshAABTreeNode`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AabNode {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub front_or_poly0: u32,
    pub back_or_poly_count: u32,
}

const ZERO_NODE: AabNode = AabNode { min: [0.0; 3], max: [0.0; 3], front_or_poly0: 0, back_or_poly_count: 0 };

/// `class AABTreeClass` as `Load_W3D` reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct AabTree {
    pub node_count: i32,
    pub poly_count: i32,
    /// `Nodes` (`node_count` entries).
    pub nodes: Arr<AabNode>,
    /// `PolyIndices` (`poly_count` entries).
    pub poly_indices: Arr<u32>,
}

/// `AABTreeClass::Load_W3D(cload)` (it reports nothing).
fn load_aabtree(cload: &mut ChunkLoad<'_>) -> W3dResult<AabTree> {
    let mut header = Stale::<{ W3dMeshAABTreeHeader::SIZE }>::new();
    cload.open_chunk()?;
    header.read(cload)?;
    cload.close_chunk()?;
    // NodeCount and PolyCount (the first 8 bytes) are used
    if header.init < 8 {
        return Err(W3dUndefined::UninitialisedRead.into());
    }
    let node_count = u32le(&header.b[0..4]) as i32;
    let poly_count = u32le(&header.b[4..8]) as i32;
    let mut tree = AabTree {
        node_count,
        poly_count,
        nodes: Arr::new(alloc(node_count, 32)?, ZERO_NODE),
        poly_indices: Arr::new(alloc(poly_count, 4)?, 0),
    };
    while cload.open_chunk()? {
        match cload.cur_chunk_id()? {
            W3D_CHUNK_AABTREE_POLYINDICES => {
                // Read_Poly_Indices: Read(PolyIndices, 4 * PolyCount), unchecked;
                // a short read copies a prefix
                let nbytes = (poly_count as u32).wrapping_mul(4);
                let f = cload.file();
                let left = f.file_size().wrapping_sub(f.position()).max(0) as usize;
                let mut buf = vec![0u8; (poly_count as usize * 4).min(left)];
                let p0 = cload.file().position();
                cload.read(&mut buf, nbytes)?;
                let copied = (i64::from(cload.file().position()) - i64::from(p0)).max(0) as usize;
                for i in 0..copied / 4 {
                    tree.poly_indices.set(i, u32le(&buf[4 * i..]));
                }
                if !copied.is_multiple_of(4) {
                    let i = copied / 4;
                    let mut w = tree.poly_indices.get(i).to_le_bytes();
                    w[..copied % 4].copy_from_slice(&buf[4 * i..copied]);
                    tree.poly_indices.set(i, u32::from_le_bytes(w));
                }
            }
            W3D_CHUNK_AABTREE_NODES => {
                // Read_Nodes: a fresh (uninitialised) w3dnode per call
                let mut w = Stale::<{ W3dMeshAABTreeNode::SIZE }>::new();
                for i in 0..node_count.max(0) as usize {
                    let ok = w.read(cload)?;
                    let n = W3dMeshAABTreeNode::from_bytes(&w.get()?);
                    let node = AabNode {
                        min: [n.min.x, n.min.y, n.min.z],
                        max: [n.max.x, n.max.y, n.max.z],
                        front_or_poly0: n.front_or_poly0,
                        back_or_poly_count: n.back_or_poly_count,
                    };
                    if ok {
                        tree.nodes.set(i, node);
                    } else {
                        // every later read fails the same way, copying nothing
                        tree.nodes.fill_from(i, node);
                        break;
                    }
                }
            }
            _ => {}
        }
        cload.close_chunk()?;
    }
    Ok(tree)
}

// ---------------------------------------------------------------------------
// The mesh
// ---------------------------------------------------------------------------

/// `WW3D::PrelitModeEnum`: which pre-lit material wrapper the loader reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrelitMode {
    Vertex = 0,
    LightmapMultiPass = 1,
    LightmapMultiTexture = 2,
}

impl Default for PrelitMode {
    /// `WW3D::PrelitMode`'s initial value (ww3d.cpp).
    fn default() -> Self {
        PrelitMode::LightmapMultiPass
    }
}

/// `MeshGeometryClass::FlagsType`.
pub mod mesh_flags {
    pub const DIRTY_BOUNDS: u32 = 0x0000_0001;
    pub const DIRTY_PLANES: u32 = 0x0000_0002;
    pub const DIRTY_VNORMALS: u32 = 0x0000_0004;
    pub const SORT: u32 = 0x0000_0010;
    pub const TWO_SIDED: u32 = 0x0000_0100;
    pub const ALIGNED: u32 = 0x0000_0200;
    pub const SKIN: u32 = 0x0000_0400;
    pub const ORIENTED: u32 = 0x0000_0800;
    pub const CAST_SHADOW: u32 = 0x0000_1000;
    pub const PRELIT_VERTEX: u32 = 0x0000_2000;
    pub const PRELIT_LIGHTMAP_MULTI_PASS: u32 = 0x0000_4000;
    pub const PRELIT_LIGHTMAP_MULTI_TEXTURE: u32 = 0x0000_8000;
    pub const ALLOW_NPATCHES: u32 = 0x0001_0000;
}
use mesh_flags::*;

/// A shader of the mesh: as read and as `ShaderClass` bits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shader {
    /// The file's struct; `None` for a legacy material's shader (#67), which
    /// `read_v3_materials` builds from `ShaderClass()` and the material.
    pub raw: Option<W3dShaderStruct>,
    pub bits: u32,
}

/// `MeshLoadContextClass::LegacyMaterialClass` (#67): one material of
/// `W3D_CHUNK_MATERIALS3`, as indices into the context's de-duplicated
/// shaders, vertex materials and textures (-1: none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LegacyMaterial {
    pub vertex_material_idx: i32,
    pub shader_idx: i32,
    pub texture_idx: i32,
}

/// `class MeshModelClass` (with its `MeshGeometryClass` part) and the
/// loading context, after `read_chunks` (see the module docs).
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    /// The header as read.
    pub header: W3dMeshHeader3Struct,
    /// `"Container.Mesh"`, or `"Mesh"` without a container name.
    pub name: Vec<u8>,
    /// `MeshGeometryClass::Flags` ([`mesh_flags`]).
    pub flags: u32,
    pub w3d_attributes: u32,
    /// `SortLevel`: a `char` upstream, so the header's value truncated to 8
    /// bits.
    pub sort_level: i8,
    pub poly_count: i32,
    pub vertex_count: i32,
    pub bound_box_min: [f32; 3],
    pub bound_box_max: [f32; 3],
    pub bound_sphere_center: [f32; 3],
    pub bound_sphere_radius: f32,
    /// The geometry arrays, allocated when both counts are not 0.
    pub vertices: Option<Arr<[f32; 3]>>,
    pub normals: Option<Arr<[f32; 3]>>,
    /// `Poly`: vertex indices, 16 bits each.
    pub polys: Option<Arr<[u16; 3]>>,
    pub surface_types: Option<Arr<u8>>,
    /// The plane equations of `read_triangles` (`Normal`, `-Dist`); upstream
    /// writes them to a scratch buffer (see the module docs).
    pub planes: Option<Arr<[f32; 4]>>,
    pub shade_indices: Option<Arr<u32>>,
    /// `VertexBoneLink`.
    pub bone_links: Option<Arr<u16>>,
    pub user_text: Option<Arr<u8>>,
    /// `CullTree` (the last AABTree chunk).
    pub cull_tree: Option<AabTree>,
    /// The mesh is collidable and has no AABTree: upstream would generate
    /// one now (not translated).
    pub needs_generated_cull_tree: bool,
    /// The context's `PrelitChunkID` (`0xFFFFFFFF` if none).
    pub prelit_chunk_id: u32,
    /// The context's material info (the last one read).
    pub mat_info: W3dMaterialInfoStruct,
    /// The context's shaders, vertex materials and textures, in load order.
    pub shaders: Vec<Shader>,
    pub vertex_materials: Vec<VertexMaterial>,
    pub textures: Vec<Texture>,
    /// The context's `LegacyMaterials` (#67).
    pub legacy_materials: Vec<LegacyMaterial>,
    /// The context's `VertexMaterialCrcs` (#67): one per vertex material a
    /// legacy material added.
    pub vertex_material_crcs: Vec<u32>,
    /// `DefMatDesc`.
    pub default_mat: MatDesc,
    /// The context's `AlternateMatDesc`.
    pub alternate_mat: MatDesc,
}

/// Options of the loader.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeshLoadOptions {
    /// `WW3D::Get_Prelit_Mode()`.
    pub prelit_mode: PrelitMode,
}

struct Ctx {
    cur_pass: i32,
    cur_tex_stage: i32,
    loaded_dig: bool,
    /// `TempUVArray`: its capacity and contents.
    temp_uv: Arr<[f32; 2]>,
    temp_uv_allocated: bool,
}

impl Mesh {
    fn set_flag(&mut self, f: u32, on: bool) {
        if on {
            self.flags |= f;
        } else {
            self.flags &= !f;
        }
    }
    fn get_flag(&self, f: u32) -> bool {
        self.flags & f != 0
    }
    /// `MeshModelClass::Peek_Vertex_Material` / `Peek_Texture` /
    /// `Peek_Shader` of the context.
    fn check_index(id: u32, count: usize) -> W3dResult<u32> {
        if (id as usize) < count {
            Ok(id)
        } else {
            Err(W3dUndefined::IndexOutOfRange.into())
        }
    }
    fn alt(&mut self, use_alt: bool) -> &mut MatDesc {
        if use_alt {
            &mut self.alternate_mat
        } else {
            &mut self.default_mat
        }
    }

    /// `MeshLoadContextClass::Add_Legacy_Material(shader, vmat, tex)`: adds
    /// the shader unless one with the same bits is there, the vertex
    /// material unless one of the same CRC is ([`vertex_material_crc`]; the
    /// name is not part of it), and the texture unless one of the same name
    /// is (`stricmp`; the asset manager returns one object per lower-cased
    /// name, so the pointer test adds nothing), and records the three
    /// indices.
    ///
    /// Kept quirk: the CRC search runs over `VertexMaterialCrcs` (only the
    /// legacy materials' vertex materials) but "not found" is tested against
    /// `VertexMaterials.Count()`. When `W3D_CHUNK_VERTEX_MATERIALS` added
    /// materials first, a new legacy material is not added and gets the
    /// index `VertexMaterialCrcs.Count()`, another material's.
    fn add_legacy_material(&mut self, shader: u32, vmat: VertexMaterial, tex: Option<Texture>) {
        let found = if plant(10081) { None } else { self.shaders.iter().position(|s| s.bits == shader) };
        let shader_idx = match found {
            Some(si) => si,
            None => {
                self.shaders.push(Shader { raw: None, bits: shader });
                self.shaders.len() - 1
            }
        };
        let legacy = vmat.legacy.expect("a legacy material");
        let crc = vertex_material_crc(&legacy);
        let vi = if plant(10088) {
            let n = self.vertex_material_crcs.len();
            (0..n).find(|&k| self.vertex_materials.get(k).map(|v| &v.name) == Some(&vmat.name)).unwrap_or(n)
        } else {
            self.vertex_material_crcs.iter().position(|&c| c == crc).unwrap_or(self.vertex_material_crcs.len())
        };
        let not_found = if plant(10082) { self.vertex_material_crcs.len() } else { self.vertex_materials.len() };
        let vertex_material_idx = if vi == not_found {
            self.vertex_materials.push(vmat);
            self.vertex_material_crcs.push(crc);
            self.vertex_materials.len() - 1
        } else {
            vi
        };
        let texture_idx = match tex {
            None => -1,
            Some(t) => {
                let same = |a: &[u8], b: &[u8]| if plant(10083) { a == b } else { a.eq_ignore_ascii_case(b) };
                match self.textures.iter().position(|x| same(&x.name, &t.name)) {
                    Some(ti) => ti as i32,
                    None => {
                        self.textures.push(t);
                        self.textures.len() as i32 - 1
                    }
                }
            }
        };
        self.legacy_materials.push(LegacyMaterial { vertex_material_idx: vertex_material_idx as i32, shader_idx: shader_idx as i32, texture_idx });
    }

    /// The legacy material `i` (`WWASSERT`s only: out of range is undefined).
    fn legacy_material(&self, i: i32) -> W3dResult<LegacyMaterial> {
        usize::try_from(i).ok().and_then(|i| self.legacy_materials.get(i).copied()).ok_or(W3dUndefined::IndexOutOfRange.into())
    }

    /// `MeshLoadContextClass::Peek_Legacy_Shader(i)`: the shader's bits.
    fn peek_legacy_shader(&self, i: i32) -> W3dResult<u32> {
        let si = self.legacy_material(i)?.shader_idx;
        Ok(self.shaders[si as usize].bits)
    }

    /// `MeshLoadContextClass::Peek_Legacy_Vertex_Material(i)`: its index, or
    /// `None` (null) for -1.
    fn peek_legacy_vertex_material(&self, i: i32) -> W3dResult<Option<u32>> {
        let vi = self.legacy_material(i)?.vertex_material_idx;
        Ok(if vi != -1 { Some(vi as u32) } else { None })
    }

    /// `MeshLoadContextClass::Peek_Legacy_Texture(i)`: its index, or `None`
    /// (null) for -1.
    fn peek_legacy_texture(&self, i: i32) -> W3dResult<Option<u32>> {
        let ti = self.legacy_material(i)?.texture_idx;
        Ok(if ti != -1 { Some(ti as u32) } else { None })
    }
}

/// `MeshModelClass::Load_W3D(cload)`, inside the mesh chunk. `Ok(None)` is
/// `WW3D_ERROR_LOAD_FAILED`.
pub fn load_w3d(cload: &mut ChunkLoad<'_>, options: &MeshLoadOptions) -> W3dResult<Option<Mesh>> {
    cload.open_chunk()?;
    if cload.cur_chunk_id()? != W3D_CHUNK_MESH_HEADER3 {
        return Ok(None);
    }
    let mut hb = [0u8; W3dMeshHeader3Struct::SIZE];
    if cload.read_into(&mut hb)? != W3dMeshHeader3Struct::SIZE as u32 {
        return Ok(None);
    }
    cload.close_chunk()?;
    let header = W3dMeshHeader3Struct::from_bytes(&hb);

    // Reset(NumTris, NumVertices, 1)
    let poly_count = header.num_tris as i32;
    let vertex_count = header.num_vertices as i32;
    let (mut vertices, mut normals, mut polys, mut surface_types) = (None, None, None, None);
    if poly_count != 0 && vertex_count != 0 {
        polys = Some(Arr::new(alloc(poly_count, 6)?, [0u16; 3]));
        surface_types = Some(Arr::new(alloc(poly_count, 1)?, 0u8));
        vertices = Some(Arr::new(alloc(vertex_count, 12)?, [0.0f32; 3]));
        normals = Some(Arr::new(alloc(vertex_count, 12)?, [0.0f32; 3]));
    }
    let default_mat = MatDesc { pass_count: 1, vertex_count, poly_count, ..MatDesc::default() };
    let alternate_mat = MatDesc { vertex_count, poly_count, ..MatDesc::default() };

    // the name: strlen of each, inside the header
    let container = c_string(&hb, 24)?;
    let mesh_name = c_string(&hb, 8)?;
    let mut name = Vec::new();
    if !container.is_empty() {
        name.extend_from_slice(container);
        name.push(b'.');
    }
    name.extend_from_slice(mesh_name);

    let mut m = Mesh {
        header,
        name,
        flags: 0,
        w3d_attributes: header.attributes,
        sort_level: header.sort_level as i8,
        poly_count,
        vertex_count,
        bound_box_min: [header.min.x, header.min.y, header.min.z],
        bound_box_max: [header.max.x, header.max.y, header.max.z],
        bound_sphere_center: [header.sph_center.x, header.sph_center.y, header.sph_center.z],
        bound_sphere_radius: header.sph_radius,
        vertices,
        normals,
        polys,
        surface_types,
        planes: None,
        shade_indices: None,
        bone_links: None,
        user_text: None,
        cull_tree: None,
        needs_generated_cull_tree: false,
        prelit_chunk_id: 0xFFFF_FFFF,
        mat_info: W3dMaterialInfoStruct::from_bytes(&[0; 16]),
        shaders: Vec::new(),
        vertex_materials: Vec::new(),
        textures: Vec::new(),
        legacy_materials: Vec::new(),
        vertex_material_crcs: Vec::new(),
        default_mat,
        alternate_mat,
    };

    // Flags
    let a = header.attributes;
    if header.version >= w3d_make_version(4, 1) {
        match a & W3D_MESH_FLAG_GEOMETRY_TYPE_MASK {
            W3D_MESH_FLAG_GEOMETRY_TYPE_CAMERA_ALIGNED => m.set_flag(ALIGNED, true),
            W3D_MESH_FLAG_GEOMETRY_TYPE_CAMERA_ORIENTED => m.set_flag(ORIENTED, true),
            W3D_MESH_FLAG_GEOMETRY_TYPE_SKIN => {
                m.set_flag(SKIN, true);
                m.set_flag(ALLOW_NPATCHES, true);
            }
            _ => {}
        }
    }
    if a & W3D_MESH_FLAG_TWO_SIDED != 0 {
        m.set_flag(TWO_SIDED, true);
    }
    if a & W3D_MESH_FLAG_CAST_SHADOW != 0 {
        m.set_flag(CAST_SHADOW, true);
    }
    if a & W3D_MESH_FLAG_NPATCHABLE != 0 {
        m.set_flag(ALLOW_NPATCHES, true);
    }
    if a & W3D_MESH_FLAG_PRELIT_MASK != 0 {
        // the selected mode, else the next one down (case fall-through)
        let mut mode = options.prelit_mode as u32;
        loop {
            match mode {
                2 if a & W3D_MESH_FLAG_PRELIT_LIGHTMAP_MULTI_TEXTURE != 0 => {
                    m.prelit_chunk_id = W3D_CHUNK_PRELIT_LIGHTMAP_MULTI_TEXTURE;
                    m.set_flag(PRELIT_LIGHTMAP_MULTI_TEXTURE, true);
                }
                1 if a & W3D_MESH_FLAG_PRELIT_LIGHTMAP_MULTI_PASS != 0 => {
                    m.prelit_chunk_id = W3D_CHUNK_PRELIT_LIGHTMAP_MULTI_PASS;
                    m.set_flag(PRELIT_LIGHTMAP_MULTI_PASS, true);
                }
                0 if a & W3D_MESH_FLAG_PRELIT_VERTEX != 0 => {
                    m.prelit_chunk_id = W3D_CHUNK_PRELIT_VERTEX;
                    m.set_flag(PRELIT_VERTEX, true);
                }
                1 | 2 => {
                    mode -= 1;
                    continue;
                }
                _ => m.prelit_chunk_id = W3D_CHUNK_PRELIT_UNLIT,
            }
            break;
        }
    } else if a & OBSOLETE_W3D_MESH_FLAG_LIGHTMAPPED != 0 {
        m.set_flag(PRELIT_LIGHTMAP_MULTI_PASS, true);
    }

    let mut ctx = Ctx {
        cur_pass: 0,
        cur_tex_stage: 0,
        loaded_dig: false,
        temp_uv: Arr::new(0, [0.0; 2]),
        temp_uv_allocated: false,
    };
    // the result of read_chunks is not used
    read_chunks(cload, &mut m, &mut ctx)?;

    // pre-3.0 skins: the bone indices gain the new root node
    if header.version < w3d_make_version(3, 0) && m.get_flag(SKIN) {
        let n = alloc(m.vertex_count, 2)?;
        let links = m.bone_links.get_or_insert_with(|| Arr::new(n, 0));
        links.map_from(0, |l| l.wrapping_add(1));
    }
    m.needs_generated_cull_tree =
        (m.w3d_attributes & W3D_MESH_FLAG_COLLISION_TYPE_MASK) >> W3D_MESH_FLAG_COLLISION_TYPE_SHIFT != 0 && m.cull_tree.is_none();
    Ok(Some(m))
}

/// The `WW3DErrorType` of the read functions: `Ok(true)` is
/// `WW3D_ERROR_OK`, `Ok(false)` `WW3D_ERROR_LOAD_FAILED`.
type Rd = W3dResult<bool>;

fn read_chunks(cload: &mut ChunkLoad<'_>, m: &mut Mesh, ctx: &mut Ctx) -> Rd {
    while cload.open_chunk()? {
        let ok = match cload.cur_chunk_id()? {
            W3D_CHUNK_VERTICES => read_vertices(cload, m)?,
            W3D_CHUNK_SURRENDER_NORMALS | W3D_CHUNK_VERTEX_NORMALS => read_vertex_normals(cload, m)?,
            W3D_CHUNK_TEXCOORDS => read_texcoords(cload, m, ctx)?,
            W3D_CHUNK_MATERIALS3 => read_v3_materials(cload, m)?,
            W3D_CHUNK_TRIANGLES => read_triangles(cload, m)?,
            W3D_CHUNK_PER_TRI_MATERIALS => read_per_tri_materials(cload, m)?,
            W3D_CHUNK_MESH_USER_TEXT => read_user_text(cload, m)?,
            W3D_CHUNK_VERTEX_COLORS => read_vertex_colors(cload, m, ctx)?,
            W3D_CHUNK_VERTEX_INFLUENCES => read_vertex_influences(cload, m)?,
            W3D_CHUNK_VERTEX_SHADE_INDICES => read_vertex_shade_indices(cload, m)?,
            W3D_CHUNK_MATERIAL_INFO => read_material_info(cload, m)?,
            W3D_CHUNK_SHADERS => read_shaders(cload, m)?,
            W3D_CHUNK_VERTEX_MATERIALS => read_vertex_materials(cload, m)?,
            W3D_CHUNK_TEXTURES => read_textures(cload, m)?,
            W3D_CHUNK_MATERIAL_PASS => read_material_pass(cload, m, ctx)?,
            W3D_CHUNK_PRELIT_UNLIT
            | W3D_CHUNK_PRELIT_VERTEX
            | W3D_CHUNK_PRELIT_LIGHTMAP_MULTI_PASS
            | W3D_CHUNK_PRELIT_LIGHTMAP_MULTI_TEXTURE => {
                read_prelit_material(cload, m, ctx)?;
                true
            }
            W3D_CHUNK_AABTREE => {
                m.cull_tree = Some(load_aabtree(cload)?);
                true
            }
            // O_W3D_CHUNK_MATERIALS(2), O_W3D_CHUNK_SURRENDER_TRIANGLES,
            // W3D_CHUNK_DEFORM, W3D_CHUNK_DAMAGE and unknown chunks: skipped
            _ => true,
        };
        cload.close_chunk()?;
        if !ok {
            return Ok(false);
        }
    }
    Ok(true)
}

/// A geometry array that must exist (`X->Get_Array()` of a null array is
/// undefined).
fn need<T>(a: &mut Option<T>) -> W3dResult<&mut T> {
    a.as_mut().ok_or(W3dUndefined::NullPointer.into())
}

fn read_vec3(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<[f32; 3]>> {
    let mut b = [0u8; 12];
    if cload.read_into(&mut b)? != 12 {
        return Ok(None);
    }
    let v = W3dVectorStruct::from_bytes(&b);
    Ok(Some([v.x, v.y, v.z]))
}

fn read_vertices(cload: &mut ChunkLoad<'_>, m: &mut Mesh) -> Rd {
    let n = m.vertex_count.max(0) as usize;
    let loc = need(&mut m.vertices)?;
    for i in 0..n {
        match read_vec3(cload)? {
            Some(v) => loc.set(i, v),
            None => return Ok(false),
        }
    }
    Ok(true)
}

fn read_vertex_normals(cload: &mut ChunkLoad<'_>, m: &mut Mesh) -> Rd {
    let n = m.vertex_count.max(0) as usize;
    let norms = need(&mut m.normals)?;
    for i in 0..n {
        match read_vec3(cload)? {
            Some(v) => norms.set(i, v),
            None => return Ok(false),
        }
    }
    Ok(true)
}

/// `Get_Temporary_UV_Array(elementcount)`: `Uninitialised_Grow`, then the
/// array (`None` for upstream's null pointer).
fn temp_uv(ctx: &mut Ctx, elementcount: i32) -> W3dResult<bool> {
    if elementcount as i64 > ctx.temp_uv.len() as i64 && elementcount > 0 {
        let n = alloc(elementcount, 8)?;
        ctx.temp_uv = Arr::new(n, [0.0; 2]);
        ctx.temp_uv_allocated = true;
    }
    Ok(ctx.temp_uv_allocated)
}

fn read_texcoords(cload: &mut ChunkLoad<'_>, m: &mut Mesh, ctx: &mut Ctx) -> Rd {
    let elementcount = (cload.cur_chunk_length()? / W3dTexCoordStruct::SIZE as u32) as i32;
    if temp_uv(ctx, elementcount)? {
        for i in 0..m.vertex_count.max(0) as usize {
            let mut b = [0u8; 8];
            if cload.read_into(&mut b)? != 8 {
                return Ok(false);
            }
            if i >= ctx.temp_uv.len() {
                return Err(W3dUndefined::IndexOutOfRange.into());
            }
            let t = W3dTexCoordStruct::from_bytes(&b);
            ctx.temp_uv.set(i, [t.u, 1.0 - t.v]);
        }
        let (pass, stage) = (ctx.cur_pass, ctx.cur_tex_stage);
        let uvs = ctx.temp_uv.clone();
        m.default_mat.install_uv_array(pass, stage, &uvs, elementcount)?;
    }
    Ok(true)
}

fn read_triangles(cload: &mut ChunkLoad<'_>, m: &mut Mesh) -> Rd {
    let n = m.poly_count.max(0) as usize;
    need(&mut m.polys)?;
    m.set_flag(DIRTY_PLANES, false);
    // get_planes: the shared scratch buffer, grown to PolyCount
    let pn = alloc(m.poly_count, 16)?;
    if m.planes.as_ref().map(|p| p.len()) != Some(pn) {
        m.planes = Some(Arr::new(pn, [0.0; 4]));
    }
    need(&mut m.surface_types)?;
    for i in 0..n {
        let mut b = [0u8; W3dTriStruct::SIZE];
        if cload.read_into(&mut b)? != W3dTriStruct::SIZE as u32 {
            return Ok(false);
        }
        let t = W3dTriStruct::from_bytes(&b);
        m.polys.as_mut().unwrap().set(i, [t.vindex[0] as u16, t.vindex[1] as u16, t.vindex[2] as u16]);
        m.planes.as_mut().unwrap().set(i, [t.normal.x, t.normal.y, t.normal.z, -t.dist]);
        m.surface_types.as_mut().unwrap().set(i, t.attributes as u8);
    }
    Ok(true)
}

/// `MeshModelClass::read_per_tri_materials(cload, context)`: unless the
/// header has one material, each kind (vertex material, texture, shader)
/// that the context holds at most one of is set for the whole of pass 0 from
/// legacy material 0; then one `uint16` legacy material id per polygon sets
/// the others, per polygon (shaders, textures) or on its three vertices
/// (vertex materials). #67: the legacy table is `read_v3_materials`'.
fn read_per_tri_materials(cload: &mut ChunkLoad<'_>, m: &mut Mesh) -> Rd {
    if m.header.num_materials == 1 {
        return Ok(true);
    }
    // get_polys(): Poly->Get_Array() of a null Poly is undefined
    need(&mut m.polys)?;
    let multi_mtl = m.vertex_materials.len() > 1;
    let multi_tex = m.textures.len() > 1;
    let multi_shad = if plant(10091) { !m.shaders.is_empty() } else { m.shaders.len() > 1 };
    if !multi_mtl {
        let v = m.peek_legacy_vertex_material(0)?;
        m.default_mat.passes[0].material = v;
    }
    if !multi_tex && !plant(10093) {
        let t = m.peek_legacy_texture(0)?;
        m.default_mat.passes[0].texture[0] = t;
    }
    if !multi_shad {
        let sh = m.peek_legacy_shader(0)?;
        m.default_mat.passes[0].shader = sh;
    }
    for i in 0..m.poly_count.max(0) as usize {
        let mut b = [0u8; 2];
        if cload.read_into(&mut b)? != 2 {
            return Ok(false);
        }
        let matid = i32::from(u16::from_le_bytes(b));
        if multi_shad {
            let sh = m.peek_legacy_shader(matid)?;
            m.default_mat.shader_array(0)?.set(i, sh);
        }
        if multi_tex {
            let t = m.peek_legacy_texture(matid)?;
            m.default_mat.texture_array(0, 0)?.set(i, t);
        }
        if multi_mtl {
            let v = m.peek_legacy_vertex_material(matid)?;
            let p = m.polys.as_ref().unwrap().get(i);
            let corners: &[u16] = if plant(10092) { &p[..1] } else { &p };
            for &vidx in corners {
                m.default_mat.set_material(usize::from(vidx), v, 0)?;
            }
        }
    }
    Ok(true)
}

/// `ShaderClass::Init_From_Material3(mat3)` (shader.cpp) on the bits of a
/// shader: a material that uses alpha disables depth writes and blends
/// `SRC_ALPHA`, `ONE_MINUS_SRC_ALPHA`.
pub fn shader_init_from_material3(bits: &mut u32, mat3: &W3dMaterial3Struct) {
    if mat3.attributes & W3DMATERIAL_USE_ALPHA != 0 {
        if !plant(10087) {
            shader_set(bits, MASK_DEPTHMASK, SHIFT_DEPTHMASK, 0); // DEPTH_WRITE_DISABLE
        }
        shader_set(bits, MASK_DSTBLEND, SHIFT_DSTBLEND, 5); // DSTBLEND_ONE_MINUS_SRC_ALPHA
        shader_set(bits, MASK_SRCBLEND, SHIFT_SRCBLEND, 2); // SRCBLEND_SRC_ALPHA
    }
}

/// `char filename[_MAX_FNAME + _MAX_EXT]` (MSVC: 256 + 256).
const MAP3_FILENAME_SIZE: usize = 512;

/// The two chunks of a legacy map (`read_v3_materials`, the same for the DC
/// and SI maps): `W3D_CHUNK_MAP3_FILENAME` into the filename buffer (a chunk
/// of the buffer's size or more is an error), then `W3D_CHUNK_MAP3_INFO`, a
/// [`W3dMap3Struct`] that must read whole (its frame count only prints a
/// debug message). `Ok(None)` is the `goto Error` (the chunks it opened are
/// left open).
fn read_map3(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<NameBuf<MAP3_FILENAME_SIZE>>> {
    let mut filename = NameBuf::<MAP3_FILENAME_SIZE>::new();
    if !cload.open_chunk()? || cload.cur_chunk_id()? != W3D_CHUNK_MAP3_FILENAME {
        return Ok(None);
    }
    let len = cload.cur_chunk_length()? as usize;
    let too_long = if plant(10089) { len > MAP3_FILENAME_SIZE } else { len >= MAP3_FILENAME_SIZE };
    if too_long {
        return Ok(None);
    }
    filename.read(cload)?;
    if !cload.close_chunk()? {
        return Ok(None);
    }
    if !cload.open_chunk()? || cload.cur_chunk_id()? != W3D_CHUNK_MAP3_INFO {
        return Ok(None);
    }
    let mut b = [0u8; W3dMap3Struct::SIZE];
    if cload.read_into(&mut b)? != W3dMap3Struct::SIZE as u32 && !plant(10094) {
        return Ok(None);
    }
    if !cload.close_chunk()? {
        return Ok(None);
    }
    Ok(Some(filename))
}

/// `WW3DAssetManager::Get_Texture(filename)` of a legacy map: the C string
/// in the buffer (undefined without a terminator in what was read).
fn get_map3_texture(filename: &NameBuf<MAP3_FILENAME_SIZE>) -> W3dResult<Option<Texture>> {
    let name = filename.string()?;
    if plant(10095) && name.is_empty() {
        return Ok(Some(Texture { name, attributes: None, mip_count: 0, no_lod: false, clamp: None }));
    }
    Ok(get_texture(name))
}

/// `MeshModelClass::read_v3_materials(cload, context)`: the header's
/// `NumMaterials` legacy materials, each a `W3D_CHUNK_MATERIAL3` holding a
/// name chunk, an info chunk (a [`W3dMaterial3Struct`]) and maps. Each
/// becomes a new vertex material (`Init_From_Material3`, named), a shader
/// (`ShaderClass()` and `Init_From_Material3`; a blending one sets the
/// mesh's `SORT` flag) and a texture: a DC map's, or an SI map's when the
/// diffuse colour is black (it also makes the shader additive and turns the
/// primary gradient off); a map enables texturing, and an untextured
/// material moves its diffuse colour to ambient. `Add_Legacy_Material` keeps
/// them; then pass 0 gets the context's first vertex material, texture and
/// shader. A missing or wrong chunk is `WW3D_ERROR_LOAD_FAILED` (`Ok(false)`),
/// leaving open the chunks opened so far.
fn read_v3_materials(cload: &mut ChunkLoad<'_>, m: &mut Mesh) -> Rd {
    for _mi in 0..m.header.num_materials {
        if !cload.open_chunk()? || cload.cur_chunk_id()? != W3D_CHUNK_MATERIAL3 {
            return Ok(false);
        }
        let mut shader = shader_reset();
        let mut tex: Option<Texture> = None;
        let mut name = NameBuf::<256>::new();

        // the material name
        if !cload.open_chunk()? || cload.cur_chunk_id()? != W3D_CHUNK_MATERIAL3_NAME {
            return Ok(false);
        }
        name.read(cload)?;
        if !cload.close_chunk()? {
            return Ok(false);
        }

        // the vertex material parameters
        if !cload.open_chunk()? || cload.cur_chunk_id()? != W3D_CHUNK_MATERIAL3_INFO {
            return Ok(false);
        }
        let mut b = [0u8; W3dMaterial3Struct::SIZE];
        if cload.read_into(&mut b)? != W3dMaterial3Struct::SIZE as u32 {
            return Ok(false);
        }
        let mat = W3dMaterial3Struct::from_bytes(&b);
        let mut vmat = D3dMaterial8::init_from_material3(&mat);
        let vname = name.string()?; // vmat->Set_Name(name)
        shader_init_from_material3(&mut shader, &mat);
        if (shader & MASK_DSTBLEND) >> SHIFT_DSTBLEND != 0 && !plant(10076) {
            m.set_flag(SORT, true);
        }
        if !cload.close_chunk()? {
            return Ok(false);
        }

        // the maps
        while cload.open_chunk()? {
            let id = cload.cur_chunk_id()?;
            if id == W3D_CHUNK_MATERIAL3_DC_MAP {
                let Some(filename) = read_map3(cload)? else { return Ok(false) };
                tex = get_map3_texture(&filename)?;
                if !plant(10090) {
                    shader_set(&mut shader, MASK_TEXTURING, SHIFT_TEXTURING, 1); // TEXTURING_ENABLE
                }
            } else if id == W3D_CHUNK_MATERIAL3_SI_MAP {
                // Vector3 == Vector3(0,0,0): every component equal
                let black = vmat.diffuse[0] == 0.0 && vmat.diffuse[1] == 0.0 && vmat.diffuse[2] == 0.0;
                if black || plant(10077) {
                    let Some(filename) = read_map3(cload)? else { return Ok(false) };
                    tex = get_map3_texture(&filename)?;
                    shader_set(&mut shader, MASK_TEXTURING, SHIFT_TEXTURING, 1); // TEXTURING_ENABLE
                    if !plant(10078) {
                        shader_set(&mut shader, MASK_DSTBLEND, SHIFT_DSTBLEND, 1); // DSTBLEND_ONE
                    }
                    shader_set(&mut shader, MASK_SRCBLEND, SHIFT_SRCBLEND, 1); // SRCBLEND_ONE
                    shader_set(&mut shader, MASK_PRIGRADIENT, SHIFT_PRIGRADIENT, 0); // GRADIENT_DISABLE
                }
            }
            cload.close_chunk()?;
        }

        // not texturing: the diffuse colour moves to ambient (the old behaviour)
        if (shader & MASK_TEXTURING) >> SHIFT_TEXTURING == 0 && !plant(10079) {
            let color = [vmat.diffuse[0], vmat.diffuse[1], vmat.diffuse[2]];
            vmat.ambient[..3].copy_from_slice(&color);
            if !plant(10080) {
                vmat.diffuse[..3].copy_from_slice(&[0.0; 3]);
            }
        }
        let info = W3dVertexMaterialStruct::from_bytes(&[0; W3dVertexMaterialStruct::SIZE]);
        let vm = VertexMaterial { name: Some(vname), info, mapper_args: [None, None], legacy: Some(vmat) };
        m.add_legacy_material(shader, vm, tex);

        // close the W3D_CHUNK_MATERIAL3
        cload.close_chunk()?;
    }

    // the defaults in the absence of an array
    if !m.vertex_materials.is_empty() {
        m.default_mat.passes[0].material = Some(0);
    }
    if !m.textures.is_empty() && !plant(10084) {
        m.default_mat.passes[0].texture[0] = Some(0);
    }
    if !m.shaders.is_empty() {
        m.default_mat.passes[0].shader = m.shaders[0].bits;
    }
    Ok(true)
}

fn read_user_text(cload: &mut ChunkLoad<'_>, m: &mut Mesh) -> Rd {
    let textlen = cload.cur_chunk_length()?;
    if m.user_text.is_some() {
        return Ok(true);
    }
    let n = alloc(textlen as i32, 1)?;
    let mut text = Arr::new(n, 0u8);
    let f = cload.file();
    let left = f.file_size().wrapping_sub(f.position()).max(0) as usize;
    let mut buf = vec![0u8; n.min(left)];
    let p0 = cload.file().position();
    let r = cload.read(&mut buf, textlen)?;
    // a short read still copies what the file had into the buffer
    let copied = (i64::from(cload.file().position()) - i64::from(p0)).max(0) as usize;
    if copied > 0 {
        text.replace(0, copied, Seg::Values(buf[..copied].to_vec()));
    }
    m.user_text = Some(text);
    Ok(r == textlen)
}

fn read_vertex_colors(cload: &mut ChunkLoad<'_>, m: &mut Mesh, ctx: &mut Ctx) -> Rd {
    if m.default_mat.color_array[0].is_none() {
        let n = m.vertex_count.max(0) as usize;
        m.default_mat.color_array(0)?;
        for i in 0..n {
            let mut b = [0u8; 4];
            if cload.read_into(&mut b)? != 4 {
                return Ok(false);
            }
            let c = W3dRGBStruct::from_bytes(&b);
            let col = [f32::from(c.r) / 255.0, f32::from(c.g) / 255.0, f32::from(c.b) / 255.0, 1.0];
            m.default_mat.color_array[0].as_mut().unwrap().set(i, dx8_convert_color(col));
        }
    }
    let p = pass_index(ctx.cur_pass)?;
    m.default_mat.passes[p].dcg_source = COLOR_SOURCE_COLOR1;
    Ok(true)
}

fn read_vertex_influences(cload: &mut ChunkLoad<'_>, m: &mut Mesh) -> Rd {
    let n = alloc(m.vertex_count, 2)?;
    let links = m.bone_links.get_or_insert_with(|| Arr::new(n, 0));
    for i in 0..n {
        let mut b = [0u8; W3dVertInfStruct::SIZE];
        if cload.read_into(&mut b)? != W3dVertInfStruct::SIZE as u32 {
            return Ok(false);
        }
        links.set(i, W3dVertInfStruct::from_bytes(&b).bone_idx);
    }
    m.set_flag(SKIN, true);
    Ok(true)
}

fn read_vertex_shade_indices(cload: &mut ChunkLoad<'_>, m: &mut Mesh) -> Rd {
    let n = alloc(m.vertex_count, 4)?;
    let si = m.shade_indices.get_or_insert_with(|| Arr::new(n, 0));
    for i in 0..n {
        let mut b = [0u8; 4];
        if cload.read_into(&mut b)? != 4 {
            return Ok(false);
        }
        si.set(i, u32::from_le_bytes(b));
    }
    Ok(true)
}

fn read_material_info(cload: &mut ChunkLoad<'_>, m: &mut Mesh) -> Rd {
    let mut b = m.mat_info.to_bytes().try_into().unwrap();
    let r = cload.read_into::<16>(&mut b)?;
    m.mat_info = W3dMaterialInfoStruct::from_bytes(&b);
    if r != 16 {
        return Ok(false);
    }
    // Set_Pass_Count on CurMatDesc, the default one while loading
    m.default_mat.pass_count = m.mat_info.pass_count as i32;
    Ok(true)
}

fn read_shaders(cload: &mut ChunkLoad<'_>, m: &mut Mesh) -> Rd {
    for _ in 0..m.mat_info.shader_count {
        let mut b = [0u8; W3dShaderStruct::SIZE];
        if cload.read_into(&mut b)? != W3dShaderStruct::SIZE as u32 {
            return Ok(false);
        }
        let raw = W3dShaderStruct::from_bytes(&b);
        m.shaders.push(Shader { raw: Some(raw), bits: convert_shader(&raw) });
    }
    Ok(true)
}

fn read_vertex_materials(cload: &mut ChunkLoad<'_>, m: &mut Mesh) -> Rd {
    while cload.open_chunk()? {
        match load_vertex_material(cload)? {
            Some(v) => m.vertex_materials.push(v),
            None => return Ok(false),
        }
        cload.close_chunk()?;
    }
    Ok(true)
}

fn read_textures(cload: &mut ChunkLoad<'_>, m: &mut Mesh) -> Rd {
    while let Some(t) = load_texture(cload)? {
        m.textures.push(t);
    }
    Ok(true)
}

fn read_material_pass(cload: &mut ChunkLoad<'_>, m: &mut Mesh, ctx: &mut Ctx) -> Rd {
    ctx.cur_tex_stage = 0;
    while cload.open_chunk()? {
        let ok = match cload.cur_chunk_id()? {
            W3D_CHUNK_VERTEX_MATERIAL_IDS => read_vertex_material_ids(cload, m, ctx)?,
            W3D_CHUNK_SHADER_IDS => read_shader_ids(cload, m, ctx)?,
            W3D_CHUNK_DCG => read_dcg(cload, m, ctx)?,
            W3D_CHUNK_DIG => read_dig(cload, m, ctx)?,
            W3D_CHUNK_SCG => true,
            W3D_CHUNK_TEXTURE_STAGE => read_texture_stage(cload, m, ctx)?,
            _ => true,
        };
        if !ok {
            return Ok(false);
        }
        cload.close_chunk()?;
    }
    ctx.cur_pass += 1;
    Ok(true)
}

fn read_vertex_material_ids(cload: &mut ChunkLoad<'_>, m: &mut Mesh, ctx: &Ctx) -> Rd {
    let pass = ctx.cur_pass;
    let use_alt = m.default_mat.has_material_data(pass)?;
    let count = m.vertex_materials.len();
    let mut vmat = Stale::<4>::new();
    if cload.cur_chunk_length()? == 4 {
        vmat.read(cload)?;
        let id = Mesh::check_index(u32le(&vmat.get()?), count)?;
        m.alt(use_alt).passes[pass_index(pass)?].material = Some(id);
    } else {
        let n = m.vertex_count.max(0) as usize;
        for i in 0..n {
            let ok = vmat.read(cload)?;
            let id = Mesh::check_index(u32le(&vmat.get()?), count)?;
            m.alt(use_alt).set_material(i, Some(id), pass)?;
            if !ok {
                let a = m.alt(use_alt).passes[pass_index(pass)?].material_array.as_mut().unwrap();
                a.fill_from(i, Some(id));
                break;
            }
        }
    }
    Ok(true)
}

fn sorts(m: &Mesh, pass: i32, bits: u32) -> bool {
    pass == 0 && (bits & MASK_DSTBLEND) >> SHIFT_DSTBLEND != 0 && (bits & MASK_ALPHATEST) >> SHIFT_ALPHATEST == 0 && i32::from(m.sort_level) == SORT_LEVEL_NONE
}

fn read_shader_ids(cload: &mut ChunkLoad<'_>, m: &mut Mesh, ctx: &Ctx) -> Rd {
    let pass = ctx.cur_pass;
    let use_alt = m.default_mat.has_shader_data(pass)?;
    let mut sid = Stale::<4>::new();
    if cload.cur_chunk_length()? == 4 {
        sid.read(cload)?;
        let id = Mesh::check_index(u32le(&sid.get()?), m.shaders.len())?;
        let bits = m.shaders[id as usize].bits;
        m.alt(use_alt).passes[pass_index(pass)?].shader = bits;
        if sorts(m, pass, bits) {
            m.set_flag(SORT, true);
        }
    } else {
        let n = m.poly_count.max(0) as usize;
        for i in 0..n {
            let ok = sid.read(cload)?;
            let id = Mesh::check_index(u32le(&sid.get()?), m.shaders.len())?;
            let bits = m.shaders[id as usize].bits;
            let arr = m.alt(use_alt).shader_array(pass)?;
            if ok {
                arr.set(i, bits);
            } else {
                arr.fill_from(i, bits);
            }
            if sorts(m, pass, bits) {
                m.set_flag(SORT, true);
            }
            if !ok {
                break;
            }
        }
    }
    Ok(true)
}

fn rgba_to_color(c: [u8; 4]) -> [f32; 4] {
    // W3dUtilityClass::Convert_Color(W3dRGBAStruct, Vector4 *)
    [f32::from(c[0]) / 255.0, f32::from(c[1]) / 255.0, f32::from(c[2]) / 255.0, f32::from(c[3]) / 255.0]
}

fn read_dcg(cload: &mut ChunkLoad<'_>, m: &mut Mesh, ctx: &Ctx) -> Rd {
    let pass = ctx.cur_pass;
    let use_alt = m.default_mat.passes[pass_index(pass)?].dcg_source != COLOR_SOURCE_MATERIAL;
    let n = m.vertex_count.max(0) as usize;
    let prelit_vertex = m.prelit_chunk_id == W3D_CHUNK_PRELIT_VERTEX;
    let md = m.alt(use_alt);
    if md.color_array[0].is_none() {
        md.color_array(0)?;
        let mut color = Stale::<4>::new();
        for i in 0..n {
            let ok = color.read(cload)?;
            let c = dx8_convert_color(rgba_to_color(color.get()?));
            let arr = md.color_array[0].as_mut().unwrap();
            if ok {
                arr.set(i, c);
            } else {
                arr.fill_from(i, c);
                break;
            }
        }
    } else if prelit_vertex {
        let mut color = Stale::<4>::new();
        for i in 0..n {
            let ok = color.read(cload)?;
            let alpha = f32::from(color.get()?[3]) / 255.0;
            let f = move |d: u32| {
                let mut col = dx8_unpack_color(d);
                col[3] = alpha;
                dx8_convert_color(col)
            };
            let arr = md.color_array[0].as_mut().unwrap();
            if ok {
                arr.set(i, f(arr.get(i)));
            } else {
                arr.map_from(i, f);
                break;
            }
        }
    }
    md.passes[pass_index(pass)?].dcg_source = COLOR_SOURCE_COLOR1;
    Ok(true)
}

fn read_dig(cload: &mut ChunkLoad<'_>, m: &mut Mesh, ctx: &mut Ctx) -> Rd {
    let pass = ctx.cur_pass;
    let use_alt = ctx.loaded_dig;
    ctx.loaded_dig = true;
    let n = m.vertex_count.max(0) as usize;
    let md = m.alt(use_alt);
    let mut color = Stale::<4>::new();
    if md.color_array[0].is_none() {
        md.color_array(0)?;
        for i in 0..n {
            let ok = color.read(cload)?;
            let c = color.get()?;
            let col = [f32::from(c[0]) / 255.0, f32::from(c[1]) / 255.0, f32::from(c[2]) / 255.0, 1.0];
            let v = dx8_convert_color(col);
            let arr = md.color_array[0].as_mut().unwrap();
            if ok {
                arr.set(i, v);
            } else {
                arr.fill_from(i, v);
                break;
            }
        }
    } else {
        for i in 0..n {
            let ok = color.read(cload)?;
            let c = color.get()?;
            let f = move |d: u32| {
                let mut col = dx8_unpack_color(d);
                col[0] *= f32::from(c[0]) / 255.0;
                col[1] *= f32::from(c[1]) / 255.0;
                col[2] *= f32::from(c[2]) / 255.0;
                dx8_convert_color(col)
            };
            let arr = md.color_array[0].as_mut().unwrap();
            if ok {
                arr.set(i, f(arr.get(i)));
            } else {
                arr.map_from(i, f);
                break;
            }
        }
    }
    md.passes[pass_index(pass)?].dcg_source = COLOR_SOURCE_COLOR1;
    Ok(true)
}

fn read_texture_stage(cload: &mut ChunkLoad<'_>, m: &mut Mesh, ctx: &mut Ctx) -> Rd {
    while cload.open_chunk()? {
        let ok = match cload.cur_chunk_id()? {
            W3D_CHUNK_TEXTURE_IDS => read_texture_ids(cload, m, ctx)?,
            W3D_CHUNK_STAGE_TEXCOORDS | W3D_CHUNK_TEXCOORDS => read_stage_texcoords(cload, m, ctx)?,
            W3D_CHUNK_PER_FACE_TEXCOORD_IDS => read_per_face_texcoord_ids(cload, m)?,
            _ => true,
        };
        if !ok {
            return Ok(false);
        }
        cload.close_chunk()?;
    }
    ctx.cur_tex_stage += 1;
    Ok(true)
}

fn read_texture_ids(cload: &mut ChunkLoad<'_>, m: &mut Mesh, ctx: &Ctx) -> Rd {
    let (pass, stage) = (ctx.cur_pass, ctx.cur_tex_stage);
    let use_alt = m.default_mat.has_texture_data(pass, stage)?;
    let count = m.textures.len();
    let mut texid = Stale::<4>::new();
    if cload.cur_chunk_length()? == 4 {
        texid.read(cload)?;
        let id = Mesh::check_index(u32le(&texid.get()?), count)?;
        m.alt(use_alt).passes[pass_index(pass)?].texture[stage_index(stage)?] = Some(id);
    } else {
        let n = m.poly_count.max(0) as usize;
        for i in 0..n {
            let ok = texid.read(cload)?;
            let t = u32le(&texid.get()?);
            if t != 0xFFFF_FFFF {
                let id = Mesh::check_index(t, count)?;
                let arr = m.alt(use_alt).texture_array(pass, stage)?;
                if ok {
                    arr.set(i, Some(id));
                } else {
                    arr.fill_from(i, Some(id));
                }
            }
            if !ok {
                break;
            }
        }
    }
    Ok(true)
}

fn read_stage_texcoords(cload: &mut ChunkLoad<'_>, m: &mut Mesh, ctx: &mut Ctx) -> Rd {
    let (pass, stage) = (ctx.cur_pass, ctx.cur_tex_stage);
    let use_alt = m.default_mat.has_uv(pass, stage)?;
    let elementcount = cload.cur_chunk_length()? / W3dTexCoordStruct::SIZE as u32;
    if temp_uv(ctx, elementcount as i32)? {
        let mut tc = Stale::<8>::new();
        for i in 0..elementcount as usize {
            let ok = tc.read(cload)?;
            let t = W3dTexCoordStruct::from_bytes(&tc.get()?);
            let v = [t.u, 1.0 - t.v];
            if ok {
                ctx.temp_uv.set(i, v);
            } else {
                // every later read fails too: the rest of the loop writes v
                ctx.temp_uv.fill_range(i, elementcount as usize, v);
                break;
            }
        }
    }
    // Install_UV_Array(pass, stage, uvs, elementcount), even for a null uvs
    // (count 0: nothing is read through it)
    let uvs = ctx.temp_uv.clone();
    m.alt(use_alt).install_uv_array(pass, stage, &uvs, elementcount as i32)?;
    Ok(true)
}

fn read_per_face_texcoord_ids(cload: &mut ChunkLoad<'_>, m: &Mesh) -> Rd {
    // sizeof(Vector3i) * Get_Polygon_Count(), in 32-bit unsigned arithmetic
    let size = 12u32.wrapping_mul(m.poly_count as u32);
    if cload.cur_chunk_length()? == size {
        cload.seek(size)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn read_prelit_material(cload: &mut ChunkLoad<'_>, m: &mut Mesh, ctx: &mut Ctx) -> Rd {
    if cload.cur_chunk_id()? == m.prelit_chunk_id {
        while cload.open_chunk()? {
            let ok = match cload.cur_chunk_id()? {
                W3D_CHUNK_MATERIAL_INFO => read_material_info(cload, m)?,
                W3D_CHUNK_VERTEX_MATERIALS => read_vertex_materials(cload, m)?,
                W3D_CHUNK_SHADERS => read_shaders(cload, m)?,
                W3D_CHUNK_TEXTURES => read_textures(cload, m)?,
                W3D_CHUNK_MATERIAL_PASS => read_material_pass(cload, m, ctx)?,
                _ => true,
            };
            cload.close_chunk()?;
            if !ok {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::w3d::synth::{chunk, chunk_sub};
    use crate::ww::chunkio::GameFile;

    /// A mesh of 4 vertices and 2 triangles (0,1,2), (0,2,3) with `num_materials`
    /// in the header, followed by `extra` chunks.
    fn legacy_mesh(num_materials: u32, extra: &[Vec<u8>]) -> Vec<u8> {
        let mut h = W3dMeshHeader3Struct::from_bytes(&[0; W3dMeshHeader3Struct::SIZE]);
        h.version = w3d_make_version(4, 2);
        h.num_tris = 2;
        h.num_vertices = 4;
        h.num_materials = num_materials;
        h.mesh_name[0] = b'm';
        let mut body = chunk(W3D_CHUNK_MESH_HEADER3, &h.to_bytes());
        body.extend(chunk(W3D_CHUNK_VERTICES, &[0u8; 48]));
        let mut tris = Vec::new();
        for t in [[0u32, 1, 2], [0, 2, 3]] {
            for v in t {
                tris.extend_from_slice(&v.to_le_bytes());
            }
            tris.extend_from_slice(&[0u8; 20]);
        }
        body.extend(chunk(W3D_CHUNK_TRIANGLES, &tris));
        for c in extra {
            body.extend_from_slice(c);
        }
        chunk_sub(W3D_CHUNK_MESH, &body)
    }

    fn load(bytes: &[u8]) -> W3dResult<Mesh> {
        let mut c = ChunkLoad::new(GameFile::new(bytes).unwrap());
        assert!(c.open_chunk().unwrap());
        Ok(load_w3d(&mut c, &MeshLoadOptions::default())?.expect("loaded"))
    }

    /// A `W3D_CHUNK_MATERIAL3`: name, info (attributes, diffuse colour,
    /// diffuse coefficients, ambient coefficients; the rest zero, opacity
    /// 0.5) and maps (`(map chunk id, file name)`).
    fn material3(name: &[u8], attributes: u32, diffuse: [u8; 3], diffuse_coef: [u8; 3], maps: &[(u32, &[u8])]) -> Vec<u8> {
        let mut info = [0u8; W3dMaterial3Struct::SIZE];
        info[0..4].copy_from_slice(&attributes.to_le_bytes());
        info[4..7].copy_from_slice(&diffuse);
        info[20..23].copy_from_slice(&diffuse_coef);
        info[32..36].copy_from_slice(&0.5f32.to_le_bytes());
        let mut nm = name.to_vec();
        nm.push(0);
        let mut b = chunk(W3D_CHUNK_MATERIAL3_NAME, &nm);
        b.extend(chunk(W3D_CHUNK_MATERIAL3_INFO, &info));
        for (id, file) in maps {
            let mut f = file.to_vec();
            f.push(0);
            let mut mb = chunk(W3D_CHUNK_MAP3_FILENAME, &f);
            mb.extend(chunk(W3D_CHUNK_MAP3_INFO, &[0, 0, 1, 0, 0, 0, 0, 0]));
            b.extend(chunk_sub(*id, &mb));
        }
        chunk_sub(W3D_CHUNK_MATERIAL3, &b)
    }

    /// #67. Predictions (written before the first run): A (alpha, red
    /// diffuse, no map) and B (A's info under another name) share one shader
    /// and one vertex material (the CRC leaves the name out), A's diffuse
    /// moved to ambient; C (a DC map "Tex.TGA") and D (black diffuse, an SI
    /// map "tex.tga") add a shader and a vertex material each and share one
    /// texture (stricmp). The alpha shader sets SORT. Pass 0 takes vertex
    /// material 0, texture 0 and shader 0; the per-triangle chunk then sees
    /// one texture (not "multi"), so pass 0's texture becomes legacy
    /// material 0's, none, and no texture array is made; the ids 2, 3 give
    /// polygon shaders 1, 2 and vertex materials 2, 1, 2, 2 (the second
    /// triangle overwrites 0 and 2).
    #[test]
    fn legacy_materials_load_dedupe_and_index() {
        let mats = [
            material3(b"a", W3DMATERIAL_USE_ALPHA, [255, 0, 0], [255, 255, 255], &[]),
            material3(b"b", W3DMATERIAL_USE_ALPHA, [255, 0, 0], [255, 255, 255], &[]),
            material3(b"c", 0, [0, 255, 0], [255, 255, 255], &[(W3D_CHUNK_MATERIAL3_DC_MAP, b"Tex.TGA")]),
            material3(b"d", 0, [0, 0, 0], [255, 255, 255], &[(W3D_CHUNK_MATERIAL3_SI_MAP, b"tex.tga")]),
        ];
        let per_tri = chunk(W3D_CHUNK_PER_TRI_MATERIALS, &[2, 0, 3, 0]);
        let m = load(&legacy_mesh(4, &[chunk_sub(W3D_CHUNK_MATERIALS3, &mats.concat()), per_tri])).unwrap();
        let li: Vec<_> = m.legacy_materials.iter().map(|l| (l.vertex_material_idx, l.shader_idx, l.texture_idx)).collect();
        assert_eq!(li, vec![(0, 0, -1), (0, 0, -1), (1, 1, 0), (2, 2, 0)]);
        assert_eq!(m.textures.len(), 1);
        assert_eq!(m.textures[0].name, b"Tex.TGA");
        assert_eq!(m.vertex_materials.len(), 3);
        assert_eq!(m.vertex_material_crcs.len(), 3);
        for (v, c) in m.vertex_materials.iter().zip(&m.vertex_material_crcs) {
            assert_eq!(vertex_material_crc(v.legacy.as_ref().unwrap()), *c);
        }
        let a = m.vertex_materials[0].legacy.unwrap();
        assert_eq!((a.ambient, a.diffuse), ([1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.5]));
        assert_eq!(m.vertex_materials[0].name.as_deref(), Some(&b"a"[..]));
        assert!(m.flags & SORT != 0);
        let alpha = m.shaders[0].bits;
        assert_eq!(((alpha & MASK_DSTBLEND) >> SHIFT_DSTBLEND, (alpha & MASK_SRCBLEND) >> SHIFT_SRCBLEND, alpha & MASK_DEPTHMASK), (5, 2, 0));
        let si = m.shaders[2].bits;
        assert_eq!(((si & MASK_DSTBLEND) >> SHIFT_DSTBLEND, (si & MASK_TEXTURING) >> SHIFT_TEXTURING, si & MASK_PRIGRADIENT), (1, 1, 0));
        assert!(m.shaders.iter().all(|s| s.raw.is_none()));
        let p = &m.default_mat.passes[0];
        assert_eq!((p.material, p.texture[0], p.shader), (Some(0), None, alpha));
        assert_eq!(p.shader_array.as_ref().unwrap().to_vec(), vec![m.shaders[1].bits, si]);
        assert!(p.texture_array[0].is_none());
        assert_eq!(p.material_array.as_ref().unwrap().to_vec(), vec![Some(2), Some(1), Some(2), Some(2)]);
        // the client's view of a legacy material
        let st = super::super::vertmaterial::VertexMaterialState::from_w3d(&m.vertex_materials[0]);
        assert_eq!((st.ambient, st.diffuse, st.opacity), ([1.0, 0.0, 0.0], [0.0; 3], 0.5));
    }

    /// #67. Predictions: a vertex material read from `W3D_CHUNK_VERTEX_MATERIALS`
    /// first makes the CRC search's "not found" (`VertexMaterials.Count()`,
    /// 1) unreachable while `VertexMaterialCrcs` is empty, so the legacy
    /// material is not added and gets index 0 (the other material); a wrong
    /// chunk inside the legacy chunk fails the read (the mesh still loads,
    /// with nothing from it); a per-triangle id past the table is undefined.
    #[test]
    fn legacy_material_quirks() {
        let mut vm = chunk(W3D_CHUNK_VERTEX_MATERIAL_NAME, b"v\0");
        vm.extend(chunk(W3D_CHUNK_VERTEX_MATERIAL_INFO, &[0u8; W3dVertexMaterialStruct::SIZE]));
        let vms = chunk_sub(W3D_CHUNK_VERTEX_MATERIALS, &chunk_sub(W3D_CHUNK_VERTEX_MATERIAL, &vm));
        let mats = chunk_sub(W3D_CHUNK_MATERIALS3, &material3(b"a", 0, [1, 2, 3], [255, 255, 255], &[]));
        let m = load(&legacy_mesh(1, &[vms, mats])).unwrap();
        assert_eq!(m.vertex_materials.len(), 1);
        assert!(m.vertex_materials[0].legacy.is_none());
        assert_eq!(m.vertex_material_crcs.len(), 0);
        assert_eq!(m.legacy_materials[0].vertex_material_idx, 0);

        let bad = chunk_sub(W3D_CHUNK_MATERIALS3, &chunk_sub(W3D_CHUNK_MATERIAL3_NAME, &[]));
        let m = load(&legacy_mesh(1, &[bad])).unwrap();
        assert!(m.legacy_materials.is_empty() && m.shaders.is_empty());

        let mats = chunk_sub(W3D_CHUNK_MATERIALS3, &[material3(b"a", 0, [1, 0, 0], [255; 3], &[]), material3(b"b", 0, [2, 0, 0], [255; 3], &[])].concat());
        let per_tri = chunk(W3D_CHUNK_PER_TRI_MATERIALS, &[0, 0, 7, 0]);
        assert_eq!(load(&legacy_mesh(2, &[mats, per_tri])), Err(W3dUndefined::IndexOutOfRange.into()));
    }

    /// #67. Predictions: `Get_Texture` refuses an empty name, so a texture
    /// chunk named "" without an info chunk ends `read_textures` (the next
    /// texture is not read), and with an info chunk it is undefined (the
    /// null texture's filter is set); a legacy DC map named "" leaves no
    /// texture but enables texturing (no move to ambient).
    #[test]
    fn empty_texture_names_give_null_textures() {
        let tex = |name: &[u8], info: bool| {
            let mut t = chunk(W3D_CHUNK_TEXTURE_NAME, name);
            if info {
                t.extend(chunk(W3D_CHUNK_TEXTURE_INFO, &[0u8; 12]));
            }
            chunk_sub(W3D_CHUNK_TEXTURE, &t)
        };
        let both = chunk_sub(W3D_CHUNK_TEXTURES, &[tex(b"\0", false), tex(b"x\0", false)].concat());
        assert!(load(&legacy_mesh(1, &[both])).unwrap().textures.is_empty());
        let with_info = chunk_sub(W3D_CHUNK_TEXTURES, &tex(b"\0", true));
        assert_eq!(load(&legacy_mesh(1, &[with_info])), Err(W3dUndefined::NullPointer.into()));

        let mats = chunk_sub(W3D_CHUNK_MATERIALS3, &material3(b"a", 0, [9, 0, 0], [255; 3], &[(W3D_CHUNK_MATERIAL3_DC_MAP, b"")]));
        let m = load(&legacy_mesh(1, &[mats])).unwrap();
        assert!(m.textures.is_empty());
        assert_eq!(m.legacy_materials[0].texture_idx, -1);
        assert_eq!((m.shaders[0].bits & MASK_TEXTURING) >> SHIFT_TEXTURING, 1);
        assert_eq!(m.vertex_materials[0].legacy.unwrap().diffuse[0], 9.0 / 255.0);
    }

    #[test]
    fn repeated_crc_feed_matches_bytewise() {
        let block = [0x12u8, 0x00, 0x80, 0x3F, 0xFF, 0xFF, 0x7F, 0x7F];
        for times in [0usize, 1, 2, 3, 7, 64, 1000, 4097] {
            for state in [0u32, 0xFFFF_FFFF, 0x1234_5678] {
                let slow = (0..times).fold(state, |s, _| realcrc::crc_feed(s, &block));
                assert_eq!(crc_feed_repeated(state, &block, times), slow, "{times} {state:x}");
            }
        }
    }
}
