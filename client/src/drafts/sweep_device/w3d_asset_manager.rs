//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DAssetManager.cpp
//!
//! Sweep 2 (#213): the team-colour ("house colour") part of `W3DAssetManager`. The render-object graph
//! (`RenderObjClass`, `MeshClass`, `HLodClass`, `MaterialInfoClass`, `TextureClass`) is not in the port, so the
//! functions work on [`RenderObjNode`], a minimal owned tree holding only what they read and write (class id, mesh
//! name, texture handles and names, vertex-material colours). Textures live in [`TextureStore`], which stands in for
//! `TextureHash` (name to texture) and the surface pixel data; surface pixels are handled as raw bytes, using the port's
//! `convert_pixel_write` (`src/drafts/ww3d/surfaceclass.rs`) and `rgb_to_hsv`/`hsv_to_rgb`
//! (`src/formats/texture/colorspace.rs`).
//!
//! Excluded (listed in the #213 comment): everything from upstream line 1372 to 1796 (`W3DAssetManager.cpp` "E&B
//! Coloring Code") is inside one `/* ... */` comment: `Make_Mesh_Unique`/`Make_HLOD_Unique` (second definitions, 1378 and
//! 1408), `Get_Texture_With_HSV_Shift`, the second `Recolor_Vertex_Material`, `Recolor_Vertices`, the second
//! `Recolor_Texture`/`Recolor_Texture_One_Time`/`Recolor_Mesh`/`Recolor_HLOD`, `Recolor_ParticleEmitter` and the second
//! `Recolor_Asset`. The Granny (`INCLUDE_GRANNY_IN_BUILD`) branches are not compiled in the retail build and are left out.
//! `Report_Used_*` only `DEBUG_LOG`; they are drafted as functions returning the lines they would log.

use rts_lockstep_engine::drafts::ww3d::surfaceclass::{convert_pixel_write, pixel_size, SurfaceDescription};
use rts_lockstep_engine::formats::texture::colorspace::{hsv_to_rgb, rgb_to_hsv};
use rts_lockstep_engine::ww::vector3::Vector3;
use std::collections::HashMap;

/// `TEAM_COLOR_PALETTE_SIZE`.
pub const TEAM_COLOR_PALETTE_SIZE: usize = 16;
/// `houseColorScale`.
pub const HOUSE_COLOR_SCALE: [u16; TEAM_COLOR_PALETTE_SIZE] = [255, 239, 223, 211, 195, 174, 167, 151, 135, 123, 107, 91, 79, 63, 47, 35];
/// `ident_scale` / `scale_epsilon` (W3DAssetManager.h).
pub const IDENT_SCALE: f32 = 1.0;
pub const SCALE_EPSILON: f32 = 0.0001;

// ---------------------------------------------------------------------------------------------------------------------
// Render object model (stand-ins for the classes the port does not have)
// ---------------------------------------------------------------------------------------------------------------------

/// Index into [`TextureStore::textures`] (a `TextureClass *`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureId(pub usize);

/// A `VertexMaterialClass` as far as `Recolor_Vertex_Material` is concerned: ambient and diffuse.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VertexMaterialColors {
    pub ambient: [f32; 3],
    pub diffuse: [f32; 3],
}

/// A `MeshClass` with its `MaterialInfoClass`.
#[derive(Clone, Debug, Default)]
pub struct MeshNode {
    /// `Get_Name()`: "mesh" or "model.mesh".
    pub name: Vec<u8>,
    /// `material->Peek_Texture(i)`; mirrored in the model by `Replace_Texture`.
    pub textures: Vec<TextureId>,
    /// `material->Peek_Vertex_Material(i)`.
    pub vertex_materials: Vec<VertexMaterialColors>,
    /// `Make_Unique` has been applied to the mesh (and, with colours, its vertex materials).
    pub unique: bool,
    pub vertex_materials_unique: bool,
}

/// `RenderObjClass::Class_ID()` cases the asset manager switches on, with their data.
#[derive(Clone, Debug)]
pub enum RenderObjNode {
    /// `CLASSID_MESH`.
    Mesh(MeshNode),
    /// `CLASSID_HLOD`: `Get_Sub_Object(i)`.
    Hlod(Vec<RenderObjNode>),
    /// Any other class id: the switches do nothing.
    Other,
}

/// A `TextureClass` reduced to what recolouring touches.
#[derive(Clone, Debug)]
pub struct TextureNode {
    /// `Get_Texture_Name()`.
    pub name: Vec<u8>,
    pub description: SurfaceDescription,
    /// Top mip surface bytes (`Get_Surface_Level()`), rows `pitch` bytes apart.
    pub pixels: Vec<u8>,
    pub pitch: usize,
    /// `Get_Mip_Level_Count()`, and the filter state copied to a recoloured texture.
    pub mip_level_count: u32,
    /// `Num_Refs()`.
    pub num_refs: i32,
}

/// `TextureHash` plus the textures it owns.
#[derive(Clone, Debug, Default)]
pub struct TextureStore {
    pub textures: Vec<TextureNode>,
    /// `TextureHash`: name to texture.
    pub hash: HashMap<Vec<u8>, TextureId>,
}

// ---------------------------------------------------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------------------------------------------------

/// `Munge_Texture_Name` (W3DAssetManager.cpp:296): `"#%d#%s"` of the colour and the lower-cased name.
pub fn munge_texture_name(oldname: &[u8], color: i32) -> Vec<u8> {
    let mut out = format!("#{}#", color).into_bytes();
    out.extend(oldname.iter().map(|b| b.to_ascii_lowercase()));
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Pixel remapping
// ---------------------------------------------------------------------------------------------------------------------

fn color_vector_scaled(color: u32) -> Vector3 {
    // Vector3 v_color((color>>16)&0xff / 255 / 255, ...): the palette variants divide by 255 twice.
    Vector3 {
        x: ((color >> 16) & 0xff) as f32 / 255.0 / 255.0,
        y: ((color >> 8) & 0xff) as f32 / 255.0 / 255.0,
        z: (color & 0xff) as f32 / 255.0 / 255.0,
    }
}

/// The 16 team-colour palette entries for `color`: the shared loop of `remapPalette16Bit`, `remapTexture16Bit`,
/// `remapPalette32Bit` and `remapTexture32Bit`. `bytes_per_pixel` is 2 or 4; entries are preset to all-ones
/// (alpha known) before `Convert_Pixel` writes the colour.
fn make_palette(sd: &SurfaceDescription, color: u32, bytes_per_pixel: usize) -> [[u8; 4]; TEAM_COLOR_PALETTE_SIZE] {
    let v_color = color_vector_scaled(color);
    let mut pal = [[0xffu8; 4]; TEAM_COLOR_PALETTE_SIZE];
    // Generate a new color gradient palette based on reference color
    for y in 0..TEAM_COLOR_PALETTE_SIZE {
        let rgb = Vector3 {
            x: HOUSE_COLOR_SCALE[y] as f32 * v_color.x,
            y: HOUSE_COLOR_SCALE[y] as f32 * v_color.y,
            z: HOUSE_COLOR_SCALE[y] as f32 * v_color.z,
        };
        let mut px = [0xffu8; 4];
        convert_pixel_write(&mut px[..bytes_per_pixel], sd, &rgb);
        pal[y] = px;
    }
    pal
}

/// `remapPalette16Bit` / `remapPalette32Bit` (W3DAssetManager.cpp:419, 540): replaces the top row's 16 palette pixels
/// (`row0`, `bytes_per_pixel` bytes each) by the house-colour gradient.
fn remap_palette(sd: &SurfaceDescription, row0: &mut [u8], color: u32, bytes_per_pixel: usize) {
    let pal = make_palette(sd, color, bytes_per_pixel);
    // check if this pixel is part of team color palette
    for p in 0..TEAM_COLOR_PALETTE_SIZE {
        row0[p * bytes_per_pixel..(p + 1) * bytes_per_pixel].copy_from_slice(&pal[p][..bytes_per_pixel]);
    }
}

/// `remapTexture16Bit` / `remapTexture32Bit` (W3DAssetManager.cpp:443, 564): for `dy` rows from `data`, any pixel equal
/// to an entry of the original `palette` becomes the matching house-colour entry (first match wins).
fn remap_texture(
    dx: usize,
    dy: usize,
    pitch_bytes: usize,
    sd: &SurfaceDescription,
    palette: &[u8],
    data: &mut [u8],
    color: u32,
    bytes_per_pixel: usize,
) {
    let pal = make_palette(sd, color, bytes_per_pixel);
    for y in 0..dy {
        for x in 0..dx {
            let off = y * pitch_bytes + x * bytes_per_pixel;
            for p in 0..TEAM_COLOR_PALETTE_SIZE {
                if palette[p * bytes_per_pixel..(p + 1) * bytes_per_pixel] == data[off..off + bytes_per_pixel] {
                    data[off..off + bytes_per_pixel].copy_from_slice(&pal[p][..bytes_per_pixel]);
                    break;
                }
            }
        }
    }
}

/// `remapAlphaTexture16Bit` (W3DAssetManager.cpp:484): ARGB 4444, `DO_HUE_SHIFT` defined. Pixels with house-colour alpha
/// (`15 - alpha`) have their hue replaced by the house colour's and saturation scaled by its saturation; alpha forced
/// opaque.
fn remap_alpha_texture_16(dx: usize, dy: usize, pitch_bytes: usize, data: &mut [u8], color: u32) {
    let v_color = [((color >> 16) & 0xff) as f32 / 255.0, ((color >> 8) & 0xff) as f32 / 255.0, (color & 0xff) as f32 / 255.0];
    let hsv_color = rgb_to_hsv(v_color);
    for y in 0..dy {
        for x in 0..dx {
            let off = y * pitch_bytes + x * 2;
            let mut pixel = u16::from_le_bytes([data[off], data[off + 1]]);
            let pixel_alpha = 15 - (pixel >> 12); // get alpha for house color
            if pixel_alpha != 0 {
                // some house color needs to show through
                let mut hsv = rgb_to_hsv([((pixel >> 8) & 0xf) as f32 / 15.0, ((pixel >> 4) & 0xf) as f32 / 15.0, (pixel & 0xf) as f32 / 15.0]);
                hsv[0] = hsv_color[0];
                hsv[1] *= hsv_color[1];
                let mut rgb = [0.0f32; 3];
                let _ = hsv_to_rgb(&mut rgb, hsv);
                pixel = (((rgb[0] * 15.0) as i32) << 8 | ((rgb[1] * 15.0) as i32) << 4 | ((rgb[2] * 15.0) as i32)) as u16;
            }
            pixel |= 0xf000; // force alpha to opaque.
            data[off..off + 2].copy_from_slice(&pixel.to_le_bytes());
        }
    }
}

/// `remapAlphaTexture32Bit` (W3DAssetManager.cpp:622): as the 16 bit version for ARGB 8888. Note upstream only writes
/// the `data[x] = ...` assignment inside the `if`, and ORs `0xff000000` always.
fn remap_alpha_texture_32(dx: usize, dy: usize, pitch_bytes: usize, data: &mut [u8], color: u32) {
    let v_color = [((color >> 16) & 0xff) as f32 / 255.0, ((color >> 8) & 0xff) as f32 / 255.0, (color & 0xff) as f32 / 255.0];
    let hsv_color = rgb_to_hsv(v_color);
    for y in 0..dy {
        for x in 0..dx {
            let off = y * pitch_bytes + x * 4;
            let mut pixel = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            let pixel_alpha = 255 - (pixel >> 24); // get alpha for house color
            if pixel_alpha != 0 {
                let mut hsv = rgb_to_hsv([((pixel >> 16) & 0xff) as f32 / 255.0, ((pixel >> 8) & 0xff) as f32 / 255.0, (pixel & 0xff) as f32 / 255.0]);
                hsv[0] = hsv_color[0];
                hsv[1] *= hsv_color[1];
                let mut rgb = [0.0f32; 3];
                let _ = hsv_to_rgb(&mut rgb, hsv);
                pixel = (((rgb[0] * 255.0) as i32) << 16 | ((rgb[1] * 255.0) as i32) << 8 | ((rgb[2] * 255.0) as i32)) as u32;
            }
            pixel |= 0xff00_0000; // force alpha to opaque.
            data[off..off + 4].copy_from_slice(&pixel.to_le_bytes());
        }
    }
}

/// `W3DAssetManager::Remap_Palette` (W3DAssetManager.cpp:619). `bits` is the locked surface, `pitch` its row pitch in
/// bytes. `do_palette_only` recolours only the top-row palette; otherwise `use_alpha` hue-shifts by alpha and the
/// default recolours the image below the top row from the palette in the top row.
pub fn remap_palette_surface(sd: &SurfaceDescription, bits: &mut [u8], pitch: usize, color: i32, do_palette_only: bool, use_alpha: bool) {
    let size = pixel_size(sd) as usize;
    let color = color as u32;
    let (w, h) = (sd.width as usize, sd.height as usize);
    if do_palette_only {
        // only recolor the palette which is stored in top row. Model only references these pixels.
        if size == 2 || size == 4 {
            remap_palette(sd, bits, color, size);
        }
    } else if use_alpha {
        if size == 2 {
            remap_alpha_texture_16(w, h, pitch, bits, color);
        } else if size == 4 {
            remap_alpha_texture_32(w, h, pitch, bits, color);
        }
    } else if size == 2 || size == 4 {
        // Recolor the image using the palette stored in top row
        let (top, rest) = bits.split_at_mut(pitch);
        let palette: Vec<u8> = top[..TEAM_COLOR_PALETTE_SIZE * size].to_vec();
        remap_texture(w, h - 1, pitch, sd, &palette, rest, color, size);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Textures
// ---------------------------------------------------------------------------------------------------------------------

impl TextureStore {
    /// `W3DAssetManager::Find_Texture(name, color)` (W3DAssetManager.cpp:373): the cached recoloured copy, if any.
    pub fn find_texture(&mut self, name: &[u8], color: i32) -> Option<TextureId> {
        let newname = munge_texture_name(name, color);
        // see if we have a cached copy
        let id = self.hash.get(&newname).copied();
        if let Some(id) = id {
            self.textures[id.0].num_refs += 1; // newtex->Add_Ref()
        }
        id
    }

    /// `W3DAssetManager::Recolor_Texture` (W3DAssetManager.cpp:395).
    pub fn recolor_texture(&mut self, texture: TextureId, color: i32) -> Option<TextureId> {
        let name = self.textures[texture.0].name.clone();
        if let Some(newtex) = self.find_texture(&name, color) {
            return Some(newtex);
        }
        self.recolor_texture_one_time(texture, color)
    }

    /// `W3DAssetManager::Recolor_Texture_One_Time` (W3DAssetManager.cpp:662): copies the texture's top surface,
    /// remaps its palette by the 4th character of the name ('D' palette only, 'A' alpha hue shift), registers the copy
    /// under the munged name. NULL for procedural (`!`-prefixed) textures.
    pub fn recolor_texture_one_time(&mut self, texture: TextureId, color: i32) -> Option<TextureId> {
        let src = self.textures[texture.0].clone();
        let name = &src.name;
        // if texture is procedural return NULL
        if !name.is_empty() && name[0] == b'!' {
            return None;
        }
        // make sure texture is loaded: TextureLoader::Request_Foreground_Loading — device-only, not translated
        let psize = pixel_size(&src.description);
        debug_assert!(psize == 2 || psize == 4, "Can't Recolor Texture");
        let mut pixels = src.pixels.clone(); // newsurf->Copy(0,0,0,0,w,h,oldsurf)
        let c3 = name.get(3).copied().unwrap_or(0);
        if c3 == b'D' || c3 == b'd' {
            remap_palette_surface(&src.description, &mut pixels, src.pitch, color, true, false); // palette stored in top row.
        } else if c3 == b'A' || c3 == b'a' {
            remap_palette_surface(&src.description, &mut pixels, src.pitch, color, false, true);
        }
        let newname = munge_texture_name(name, color);
        let newtex = TextureNode {
            name: newname.clone(),
            description: src.description,
            pixels,
            pitch: src.pitch,
            mip_level_count: src.mip_level_count,
            num_refs: 2, // NEW_REF + TextureHash.Insert's Add_Ref
        };
        self.textures.push(newtex);
        let id = TextureId(self.textures.len() - 1);
        self.hash.insert(newname, id);
        Some(id)
    }

    /// `W3DAssetManager::Report_Used_Textures` (W3DAssetManager.cpp:1310): the `DEBUG_LOG` lines for textures with
    /// more than the hash's own reference (the release code is commented out upstream).
    pub fn report_used_textures(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for tex in self.hash.values().map(|id| &self.textures[id.0]) {
            if tex.num_refs > 1 {
                lines.push(format!("**Texture \"{}\" referenced {} times on map reset", String::from_utf8_lossy(&tex.name), tex.num_refs - 1));
            }
        }
        lines
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Replacing and recolouring assets
// ---------------------------------------------------------------------------------------------------------------------

/// `W3DAssetManager::replaceAssetTexture` (W3DAssetManager.cpp:305).
pub fn replace_asset_texture(robj: &mut RenderObjNode, old_tex: TextureId, new_tex: TextureId) -> i32 {
    match robj {
        RenderObjNode::Mesh(m) => replace_mesh_texture(m, old_tex, new_tex),
        RenderObjNode::Hlod(subs) => replace_hlod_texture(subs, old_tex, new_tex),
        RenderObjNode::Other => 0,
    }
}

/// `W3DAssetManager::replaceHLODTexture` (W3DAssetManager.cpp:319): `didReplace |=` over the sub objects.
pub fn replace_hlod_texture(subs: &mut [RenderObjNode], old_tex: TextureId, new_tex: TextureId) -> i32 {
    let mut did_replace = 0;
    for sub in subs.iter_mut() {
        did_replace |= replace_asset_texture(sub, old_tex, new_tex);
    }
    did_replace
}

/// `W3DAssetManager::replaceMeshTexture` (W3DAssetManager.cpp:333): `model->Replace_Texture` and
/// `material->Replace_Texture(i, newTex)` for every slot holding `oldTex`.
pub fn replace_mesh_texture(mesh: &mut MeshNode, old_tex: TextureId, new_tex: TextureId) -> i32 {
    let mut did_replace = 0;
    for slot in mesh.textures.iter_mut() {
        if *slot == old_tex {
            *slot = new_tex;
            did_replace = 1;
        }
    }
    did_replace
}

/// `W3DAssetManager::replacePrototypeTexture` (W3DAssetManager.cpp:363): `Get_Texture(old)`/`Get_Texture(new)` are
/// the caller's lookups (`TextureStore` has no loader), passed in.
pub fn replace_prototype_texture(robj: &mut RenderObjNode, old_tex: TextureId, new_tex: TextureId) -> i32 {
    replace_asset_texture(robj, old_tex, new_tex)
}

/// `W3DAssetManager::Recolor_Vertex_Material` (W3DAssetManager.cpp:980): ambient and diffuse set to the colour.
pub fn recolor_vertex_material(vmat: &mut VertexMaterialColors, color: i32) {
    let rgb = [((color >> 16) & 0xff) as f32 / 255.0, ((color >> 8) & 0xff) as f32 / 255.0, (color & 0xff) as f32 / 255.0];
    // We ignore the existing ambient/diffuse and assume they were 1.0.
    vmat.ambient = rgb;
    vmat.diffuse = rgb;
}

fn starts_with_ignore_case(s: &[u8], prefix: &[u8]) -> bool {
    s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// The mesh-name test shared by `Recolor_Mesh` and `getMeshColorMethods`: the part after the first '.' (if there is
/// one and a character follows it), else the whole name, starts with "HOUSECOLOR" (case-insensitive, 10 characters).
fn mesh_name_is_housecolor(name: &[u8]) -> bool {
    // (meshName=strchr(name,'.')) != 0 && *(meshName++)  -> after the dot; falls to the whole name if the dot is absent
    // or the dot is the end of the string is not possible for *(meshName++) (the '.' itself is non-zero).
    let mesh_name: &[u8] = match name.iter().position(|&b| b == b'.') {
        Some(p) => &name[p + 1..],
        None => name,
    };
    starts_with_ignore_case(mesh_name, b"HOUSECOLOR")
}

/// `W3DAssetManager::Recolor_Mesh` (W3DAssetManager.cpp:919): house-colour vertex materials when the mesh name says
/// HOUSECOLOR; every "ZHC" texture replaced by its recoloured copy.
pub fn recolor_mesh(store: &mut TextureStore, mesh: &mut MeshNode, color: i32) -> i32 {
    let mut did_recolor = 0;
    // recolor vertex material (assuming mesh is housecolor)
    if mesh_name_is_housecolor(&mesh.name) {
        for vm in mesh.vertex_materials.iter_mut() {
            recolor_vertex_material(vm, color);
        }
        did_recolor = 1;
    }
    // recolor textures
    for i in 0..mesh.textures.len() {
        let oldtex = mesh.textures[i];
        if starts_with_ignore_case(&store.textures[oldtex.0].name, b"ZHC") {
            // This texture needs to be adjusted for housecolor
            if let Some(newtex) = store.recolor_texture(oldtex, color) {
                mesh.textures[i] = newtex; // model->Replace_Texture(oldtex,newtex); material->Replace_Texture(i,newtex)
                did_recolor = 1;
            }
        }
    }
    did_recolor
}

/// `W3DAssetManager::Recolor_HLOD` (W3DAssetManager.cpp:964).
pub fn recolor_hlod(store: &mut TextureStore, subs: &mut [RenderObjNode], color: i32) -> i32 {
    let mut did_recolor = 0;
    for sub in subs.iter_mut() {
        did_recolor |= recolor_asset(store, sub, color);
    }
    did_recolor
}

/// `W3DAssetManager::Recolor_Asset` (W3DAssetManager.cpp:903).
pub fn recolor_asset(store: &mut TextureStore, robj: &mut RenderObjNode, color: i32) -> i32 {
    match robj {
        RenderObjNode::Mesh(m) => recolor_mesh(store, m, color),
        RenderObjNode::Hlod(subs) => recolor_hlod(store, subs, color),
        RenderObjNode::Other => 0,
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Uniqing
// ---------------------------------------------------------------------------------------------------------------------

/// `getMeshColorMethods` (W3DAssetManager.cpp:1198): `(either, vertexColor, textureColor)`; upstream returns `either` and
/// sets the other two through references.
pub fn get_mesh_color_methods(mesh: &MeshNode, store: &TextureStore) -> (bool, bool, bool) {
    let mut vertex_color = false;
    let mut texture_color = false;
    // Check if mesh is using custom texture containing house color
    for t in &mesh.textures {
        if starts_with_ignore_case(&store.textures[t.0].name, b"ZHC") {
            texture_color = true;
            break;
        }
    }
    // Check if this object has housecolors on mesh
    if mesh_name_is_housecolor(&mesh.name) {
        vertex_color = true;
    }
    (vertex_color || texture_color, vertex_color, texture_color)
}

/// `W3DAssetManager::Make_Mesh_Unique` (W3DAssetManager.cpp:1232).
pub fn make_mesh_unique(store: &TextureStore, mesh: &mut MeshNode, geometry: bool, colors: bool) {
    // figure out what type of coloring this mesh requires (if any)
    let (any, is_vertex_color, _is_texture_color) = get_mesh_color_methods(mesh, store);
    if (colors && any) || geometry {
        // mesh has some house color applied so make those components unique to mesh.
        if !geometry {
            // scaling geometry automatically makes it unique so not needed here.
            mesh.unique = true; // mesh->Make_Unique()
        }
        if colors && is_vertex_color {
            mesh.vertex_materials_unique = true; // Peek_Vertex_Material(i)->Make_Unique() for each
        }
    }
}

/// `W3DAssetManager::Make_HLOD_Unique` (W3DAssetManager.cpp:1173).
pub fn make_hlod_unique(store: &TextureStore, subs: &mut [RenderObjNode], geometry: bool, colors: bool) {
    for sub in subs.iter_mut() {
        make_unique(store, sub, geometry, colors);
    }
}

/// `W3DAssetManager::Make_Unique`.
pub fn make_unique(store: &TextureStore, robj: &mut RenderObjNode, geometry: bool, colors: bool) {
    match robj {
        RenderObjNode::Mesh(m) => make_mesh_unique(store, m, geometry, colors),
        RenderObjNode::Hlod(subs) => make_hlod_unique(store, subs, geometry, colors),
        RenderObjNode::Other => {}
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Reports (DEBUG_LOG only)
// ---------------------------------------------------------------------------------------------------------------------

/// `W3DAssetManager::Report_Used_Prototypes` (W3DAssetManager.cpp:1263): one log line per HLOD or mesh prototype still
/// registered. `prototype_names` are the names of those prototypes.
pub fn report_used_prototypes(prototype_names: &[Vec<u8>]) -> Vec<String> {
    // while (count-- > 0): from the last prototype to the first.
    prototype_names.iter().rev().map(|n| format!("**Unfreed Prototype On Map Reset: {}", String::from_utf8_lossy(n))).collect()
}

/// `W3DAssetManager::Report_Used_FontChars` (W3DAssetManager.cpp:1293): one line per font chars with `Num_Refs() >= 1`.
pub fn report_used_font_chars(fonts: &[(Vec<u8>, i32)]) -> Vec<String> {
    fonts
        .iter()
        .rev()
        .filter(|(_, refs)| *refs >= 1)
        .map(|(n, _)| format!("**Unfreed FontChar On Map Reset: {}", String::from_utf8_lossy(n)))
        .collect()
}

/// `W3DAssetManager::Report_Used_Font3DDatas` (W3DAssetManager.cpp:1348): one line per font data whose count is not 1.
pub fn report_used_font3d_datas(fonts: &[(Vec<u8>, i32)]) -> Vec<String> {
    fonts
        .iter()
        .filter(|(_, refs)| *refs != 1)
        .map(|(n, _)| format!("**Unfreed Font3DDatas On Map Reset: {}", String::from_utf8_lossy(n)))
        .collect()
}

/// `W3DAssetManager::Report_Used_Assets` (W3DAssetManager.cpp:1279): the prototypes, textures, font 3D data and font
/// chars reports in that order.
pub fn report_used_assets(
    prototype_names: &[Vec<u8>],
    store: &TextureStore,
    font3d: &[(Vec<u8>, i32)],
    font_chars: &[(Vec<u8>, i32)],
) -> Vec<String> {
    let mut lines = report_used_prototypes(prototype_names);
    // todo upstream: report unfreed skeletons and animations
    lines.extend(store.report_used_textures());
    lines.extend(report_used_font3d_datas(font3d));
    lines.extend(report_used_font_chars(font_chars));
    lines
}
